//! **Gate 0** — a sanitização dos metadados de um GGUF, sobre os **bytes crus**.
//!
//! A classe de vulnerabilidade que este módulo fecha tem um só nome: o leitor
//! **confia nos metadados declarados** e aloca ou lê com base neles antes de
//! confirmar que correspondem aos dados que o ficheiro realmente tem. Seis CVEs
//! reais são desta classe, e cada limite abaixo existe por causa de uma delas.
//!
//! # Porque isto é sobre bytes e **não** sobre um parser
//!
//! A validação tem de acontecer **antes** do parse — é o que a torna útil. Um
//! validador construído sobre um parser roda depois dele, e nessa altura o
//! parser já alocou. A alegação não é teórica: foi medida em `ggus` 0.5.1, o
//! parser GGUF candidato, e o resultado está em [`SanitizeLimits`]: um ficheiro
//! de **24 bytes** que declara `2^40` pares chave-valor faz `GGuf::new` pedir
//! **19 791 209 299 984 bytes** (≈ 19,8 TB) *antes de ler o primeiro par*,
//! porque o parser usa a contagem declarada como capacidade de um `IndexMap`.
//! Validar depois disso é validar depois do dano.
//!
//! Este módulo não descodifica valor nenhum em tipos do domínio: ele **mede**.
//! Percorre o cabeçalho, a secção de pares chave-valor e os descritores de
//! tensor com aritmética verificada, e verifica que tudo o que o ficheiro
//! declara cabe nos bytes que ele tem. Não há um `Vec` nem um `String` na
//! caminhada — só o `&[u8]` que o chamador entregou.
//!
//! # Os dois orçamentos, e porque são dois
//!
//! [`SanitizeLimits::max_metadata_size`] limita os bytes **lidos do ficheiro**;
//! [`SanitizeLimits::max_decoded_metadata_size`] limita os bytes que um leitor
//! que materialize os metadados **alocaria**. Os dois números são independentes
//! porque a codificação de fio é compacta e a memória não: um array de `U8`
//! ocupa 1 byte por elemento no ficheiro e 8 por elemento quando cada elemento
//! vira um escalar uniforme; um array de strings ocupa 8 bytes de prefixo por
//! elemento mais o texto, e o texto é que sobrevive à descodificação.
//!
//! Consequência com os limites por omissão, escrita com números **medidos**
//! (teste `the_decoded_budget_is_reachable_with_the_default_limits`):
//! [`SanitizeLimits::max_array_elements`] impede um **único** array de chegar
//! aos 256 MiB descodificados — 1 000 000 elementos `U8` são 1 MB lidos e 8 MB
//! descodificados —, então quem chega ao orçamento descodificado é a **soma**.
//! Com 33 arrays de um milhão de elementos `U8` o ficheiro passa (264 000 000
//! bytes descodificados); com 34, a recusa é do orçamento descodificado
//! (272 000 000 > 268 435 456), com os bytes lidos a ficarem em ≈34 MB, muito
//! abaixo do orçamento deles. Do outro lado, uma secção de metadados acima de
//! 256 MiB estoura o orçamento dos bytes **lidos** sem que o descodificado seja
//! sequer alcançado. Os testes fixam os dois casos, cada um com o outro
//! orçamento folgado.
//!
//! # O que foi reaproveitado
//!
//! Os 24 bytes do cabeçalho **não** são relidos aqui: a caminhada começa por
//! [`crate::gguf::parse_header`] (`gguf.rs:192`), que já tem os seus testes, e
//! traduz o relatório dele em [`Rejection`]. O que este módulo acrescenta é o
//! percurso da secção que vem depois do cabeçalho — que [`parse_header`] não
//! percorre, como o próprio módulo declara (`gguf.rs:46-51`).
//!
//! # O que o Gate 0 **não** prova
//!
//! - **Não valida a extensão dos dados dos tensores.** O tamanho em bytes de um
//!   tensor depende da tabela de tipos GGML (blocos de quantização, `Q4_K` com
//!   256 elementos por bloco, etc.), que é uma tabela de dezenas de entradas do
//!   `llama.cpp`. O que ele valida é o **deslocamento** declarado de cada
//!   tensor: um `offset` que aponta para fora da secção de dados é recusado. Um
//!   tensor cujo `offset` está dentro mas cujo `offset + nbytes` não cabe passa
//!   por aqui — e essa é a lacuna que fica declarada em vez de escondida.
//! - **Não valida semântica.** Um `general.architecture` que diga `"gpt"` num
//!   ficheiro de visão passa: o que se mede é estrutura, não sentido.
//! - **Não substitui o [`crate::gguf::verify_model_digest`].** O Gate 0 diz "a
//!   estrutura declarada cabe nos bytes"; quem diz *qual* ficheiro é continua a
//!   ser o digest.
//!
//! # A aritmética
//!
//! Todo o cálculo de fim de campo passa por [`usize::try_from`] e
//! [`usize::checked_add`]/[`u64::checked_mul`]; nenhuma soma de `offset + len` é
//! feita com `+`. Onde a aritmética puder falhar, a resposta é
//! [`Rejection::ArithmeticOverflow`], não um pânico — é o que fecha a classe do
//! CVE-2026-86289 (`readGGUFV1String` calculava o fim da string em `usize` sem
//! verificação) e do CVE-2025-53630 (`gguf_init_from_file_impl`).

use std::fmt;

use serde::Serialize;

use arkhe_verify_wasm::encoding::encode_hex;

use crate::gguf::{parse_header, HEADER_LEN, SUPPORTED_VERSIONS, GGUF_MAGIC};

/// A largura do prefixo de comprimento de uma string GGUF: um `u64`.
const STRING_LEN_PREFIX: u64 = 8;

/// A largura do cabeçalho de um array GGUF: o tipo do elemento (`u32`) mais a
/// contagem (`u64`).
const ARRAY_HEADER_LEN: u64 = 12;

/// O custo, em bytes materializados, de **um** valor escalar descodificado.
///
/// Não é o tamanho do tipo no ficheiro (`U8` ocupa 1 byte lá) e sim o mínimo que
/// um leitor que descodifique para uma representação uniforme de valor ocupa em
/// memória. É um **piso deliberado**, não uma estimativa: um `enum` de valor
/// com discriminante mais a carga mais larga não desce de 8 bytes, e o orçamento
/// existe para limitar memória, então arredondar para baixo é o lado seguro.
const DECODED_SCALAR_WIDTH: u64 = 8;

/// Os bytes mínimos que um par chave-valor ocupa no ficheiro: o prefixo do
/// comprimento da chave (8), o tipo do valor (4) e um valor escalar de 1 byte.
///
/// Serve para recusar uma contagem declarada que não cabe no ficheiro **antes**
/// de iterar sobre ela — a defesa do CVE-2026-5757 e do CVE-2026-7482.
const MIN_KV_BYTES: u64 = STRING_LEN_PREFIX + 4 + 1;

/// Os bytes mínimos que um descritor de tensor ocupa no ficheiro: o prefixo do
/// nome (8), o número de dimensões (4), o tipo (4) e o deslocamento (8).
///
/// Zero dimensões é um valor legal, então não há bytes de dimensão no mínimo.
const MIN_TENSOR_BYTES: u64 = STRING_LEN_PREFIX + 4 + 4 + 8;

/// O alinhamento da secção de dados quando o ficheiro não traz
/// `general.alignment`. É o mesmo valor por omissão do `llama.cpp` e do `ggus`.
const DEFAULT_ALIGNMENT: u64 = 32;

/// A chave de metadados que declara o alinhamento da secção de dados, em bytes.
const GENERAL_ALIGNMENT: &[u8] = b"general.alignment";

/// Os limites do Gate 0, com os valores por omissão que a tabela de CVEs
/// justifica.
///
/// Cada limite é um `u64` e é comparado com o valor declarado pelo ficheiro
/// **antes** de o valor ser usado para percorrer, alocar ou contar. Comparar
/// (`found > max`) é a operação inteira: nenhum limite é aplicado depois de o
/// dado declarado ter sido consumido.
///
/// # Porque não se usa o `ggus`, medido
///
/// A decisão de validar bytes em vez de usar o parser `ggus` 0.5.1 foi tomada
/// sobre medições, não sobre preferência. As três que pesaram:
///
/// 1. **O parser aloca a partir da contagem declarada.** `ggus-0.5.1/src/file.rs`
///    faz `IndexMap::with_capacity(header.metadata_kv_count as _)` com o `u64`
///    que veio do ficheiro. Medido com uma sonda própria: um ficheiro de 24
///    bytes com `metadata_kv_count = 2^40` aborta o processo com
///    `memory allocation of 19791209299984 bytes failed`; com `2^32` declarados,
///    pede 77 309 411 344 bytes. É a classe do CVE-2026-65315 *dentro* do
///    parser, e significa que validar depois de `GGuf::new` seria tarde.
/// 2. **O parser entra em pânico com entrada construída.** O mesmo `file.rs`
///    termina com `&data[..data_len]` sem verificar que `data_len` cabe nos
///    bytes que sobraram. Um ficheiro de 64 bytes que declara um tensor de 16
///    bytes de dado inexistente aborta com
///    `range end index 16 out of range for slice of length 0` (exit 101). Um
///    gate que possa ser transformado num pânico por um ficheiro malformado não
///    é um gate.
/// 3. **A alegação de DoS no header não se reproduziu.** O rascunho externo diz
///    que `read_header()` faz cast dos primeiros 24 bytes para
///    `#[repr(C)] GGufFileHeader` sem validação. Refutado: `header.rs` chama
///    `self.skip::<GGufFileHeader>(1)?` **antes** do `unsafe { ptr.read() }`, e
///    `skip` usa `split_at_checked`. Medido em comprimentos 0, 1, 3, 4, 8, 16 e
///    23: `Err(Eos)` em todos. O que sobra é um desalinhamento real — o cast
///    para um tipo de alinhamento 8 sobre uma slice com um deslocamento ímpar é
///    UB pela regra da linguagem, ainda que o x86 o tolere e não se tenha
///    observado falha. Não é um DoS reproduzível.
///
/// Dois argumentos que foram considerados e **descartados** por medição, para
/// não ficar a impressão de que a decisão se apoia neles: o `ggus` **compila**
/// para `wasm32-unknown-unknown` (verificado, apesar de trazer `rayon` e
/// `crossbeam`) e o `GGuf::new` dele **aceita** a fixture real de 24 bytes. O
/// peso extra de dependências (mais de 40 crates, `regex`, `num_enum`,
/// `ggml-quants`, 15 sítios `unsafe` e `alloc`/`dealloc` crus) é um custo real,
/// mas não é o argumento — o argumento é o 1 e o 2.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SanitizeLimits {
    /// O comprimento máximo, em bytes, de uma string de metadados (chave, valor
    /// ou elemento de array). **65 535.**
    ///
    /// Mitiga o CVE-2026-5757 (metadados confiados sem validação) e o
    /// CVE-2026-86289 (o comprimento da string somado ao deslocamento sem
    /// verificação).
    pub max_string_len: u64,
    /// O número máximo de elementos que um array de metadados pode declarar.
    /// **1 000 000.**
    ///
    /// Mitiga o CVE-2026-65315 (alocação descontrolada no parser de metadados):
    /// a contagem declarada é comparada com este limite antes de qualquer
    /// elemento ser percorrido.
    pub max_array_elements: u64,
    /// O número máximo de pares chave-valor que o cabeçalho pode declarar.
    /// **100 000.**
    ///
    /// Mitiga o CVE-2026-65315. É a primeira barreira: uma contagem declarada
    /// acima disto é recusada sem tocar no resto do ficheiro.
    pub max_kv_pairs: u64,
    /// O número máximo de dimensões de um tensor. **4.**
    ///
    /// Mitiga o CVE-2026-53923 (truncamento inteiro das dimensões de tensor nos
    /// kernels de `dequantize`). O campo é um `u32` no fio, mas o limite é
    /// aplicado em `u64` para que a comparação não dependa da largura da
    /// plataforma.
    pub max_tensor_dims: u64,
    /// O número máximo de tensores que o cabeçalho pode declarar. **1 000 000.**
    ///
    /// Mitiga o CVE-2026-65315, pelo mesmo mecanismo de [`Self::max_kv_pairs`].
    pub max_tensor_count: u64,
    /// O tamanho máximo, em bytes, da secção de metadados e de descritores de
    /// tensor **lidos do ficheiro**. **256 MiB** (268 435 456).
    ///
    /// Mitiga o CVE-2026-7482 ("Bleeding Llama", heap OOB read no loader, CVSS
    /// 9.1). Não limita o tamanho do **ficheiro** — um modelo de vários GB tem
    /// uma secção de metadados pequena, e recusar pelo tamanho do ficheiro
    /// recusaria modelos legítimos.
    pub max_metadata_size: u64,
    /// O tamanho máximo, em bytes, dos metadados **materializados** por um
    /// leitor que os descodifique. **256 MiB** (268 435 456).
    ///
    /// Independente de [`Self::max_metadata_size`] — ver a nota do módulo sobre
    /// os dois orçamentos. Mitiga o CVE-2026-7482 e o CVE-2026-65315: é este
    /// orçamento que impede que um array compacto no ficheiro expanda para
    /// gigabytes em memória.
    pub max_decoded_metadata_size: u64,
    /// O número máximo de arrays **abertos ao mesmo tempo** durante o percurso,
    /// contando o array mais exterior como 1. **64.**
    ///
    /// Mitiga o CVE-2026-5757. É também o limite de recursão do percurso, e é
    /// por isso que ele existe: cada nível de aninhamento custa 12 bytes no
    /// ficheiro, então sem um limite um atacante chega à pilha por um preço
    /// trivial. Quem levante este limite está a levantar a profundidade máxima
    /// da recursão — o valor por omissão é o que mantém o percurso seguro.
    pub max_array_nesting: u64,
    /// O valor máximo de uma dimensão de tensor. **`u32::MAX`** (4 294 967 295).
    ///
    /// Acrescentado além da tabela inicial, com base no CVE-2026-53923: as
    /// dimensões são `u64` no ficheiro e um kernel de `dequantize` que as receba
    /// como `u32` **trunca**. Recusar a dimensão acima de `u32::MAX` recusa o
    /// valor que causa o truncamento, em vez de deixá-lo passar para quem o leia
    /// de forma mais estreita.
    pub max_tensor_dim_value: u64,
}

impl Default for SanitizeLimits {
    /// A tabela de limites documentada em [`SanitizeLimits`].
    fn default() -> Self {
        Self {
            max_string_len: 65_535,
            max_array_elements: 1_000_000,
            max_kv_pairs: 100_000,
            max_tensor_dims: 4,
            max_tensor_count: 1_000_000,
            max_metadata_size: 256 * 1024 * 1024,
            max_decoded_metadata_size: 256 * 1024 * 1024,
            max_array_nesting: 64,
            max_tensor_dim_value: u32::MAX as u64,
        }
    }
}

/// Um campo do ficheiro, para nomear o que falhou sem depender de uma posição
/// numérica.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Field {
    /// O magic de 4 bytes, no deslocamento 0.
    Magic,
    /// O campo de versão (`u32`), no deslocamento 4.
    Version,
    /// O contador de tensores (`u64`), no deslocamento 8.
    TensorCount,
    /// O contador de pares chave-valor (`u64`), no deslocamento 16.
    KvCount,
    /// O prefixo de comprimento (`u64`) de uma string.
    StringLength,
    /// Os bytes de uma string.
    StringBytes,
    /// O campo de tipo (`u32`) de um valor de metadados.
    ValueType,
    /// O corpo de um valor de metadados.
    Value,
    /// O cabeçalho (`u32` tipo + `u64` contagem) de um array.
    ArrayHeader,
    /// Um elemento de um array de metadados.
    ArrayElement,
    /// O número de dimensões (`u32`) de um descritor de tensor.
    TensorDims,
    /// Uma dimensão (`u64`) de um descritor de tensor.
    TensorDimension,
    /// O tipo (`u32`) de um tensor.
    TensorType,
    /// O deslocamento (`u64`) de um tensor dentro da secção de dados.
    TensorOffset,
    /// O alinhamento declarado por `general.alignment`.
    Alignment,
}

impl Field {
    /// O nome do campo, como aparece no diagnóstico.
    fn label(self) -> &'static str {
        match self {
            Field::Magic => "magic",
            Field::Version => "versão",
            Field::TensorCount => "contador de tensores",
            Field::KvCount => "contador de pares chave-valor",
            Field::StringLength => "comprimento de string",
            Field::StringBytes => "bytes de string",
            Field::ValueType => "tipo do valor",
            Field::Value => "valor",
            Field::ArrayHeader => "cabeçalho de array",
            Field::ArrayElement => "elemento de array",
            Field::TensorDims => "número de dimensões",
            Field::TensorDimension => "dimensão",
            Field::TensorType => "tipo do tensor",
            Field::TensorOffset => "deslocamento do tensor",
            Field::Alignment => "alinhamento",
        }
    }
}

/// Um dos limites de [`SanitizeLimits`], pelo nome que tem na configuração.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Limit {
    /// `max_string_len` — CVE-2026-5757, CVE-2026-86289.
    StringLength,
    /// `max_array_elements` — CVE-2026-65315.
    ArrayElements,
    /// `max_kv_pairs` — CVE-2026-65315.
    KvPairs,
    /// `max_tensor_dims` — CVE-2026-53923.
    TensorDims,
    /// `max_tensor_count` — CVE-2026-65315.
    TensorCount,
    /// `max_metadata_size` — CVE-2026-7482.
    MetadataSize,
    /// `max_decoded_metadata_size` — CVE-2026-7482, CVE-2026-65315.
    DecodedMetadataSize,
    /// `max_array_nesting` — CVE-2026-5757.
    ArrayNesting,
    /// `max_tensor_dim_value` — CVE-2026-53923.
    TensorDimValue,
}

impl Limit {
    /// O nome do limite, igual ao campo de [`SanitizeLimits`] que o define.
    fn label(self) -> &'static str {
        match self {
            Limit::StringLength => "max_string_len",
            Limit::ArrayElements => "max_array_elements",
            Limit::KvPairs => "max_kv_pairs",
            Limit::TensorDims => "max_tensor_dims",
            Limit::TensorCount => "max_tensor_count",
            Limit::MetadataSize => "max_metadata_size",
            Limit::DecodedMetadataSize => "max_decoded_metadata_size",
            Limit::ArrayNesting => "max_array_nesting",
            Limit::TensorDimValue => "max_tensor_dim_value",
        }
    }
}

/// Porque o Gate 0 recusou um ficheiro.
///
/// É um tipo e não uma frase porque a razão é a informação que o chamador
/// precisa: um teste que afirme "recusado" sem dizer porquê passa quando o
/// ficheiro é recusado pela razão errada — e um teste negativo que passa quando
/// devia falhar é pior que nenhum. Cada variante é um modo de falha distinto, e
/// os testes afirmam a variante exata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Rejection {
    /// Os quatro primeiros bytes não são `GGUF`.
    BadMagic {
        /// Os bytes encontrados, em hex.
        found: String,
    },
    /// A versão declarada não é interpretada — as mesmas versões que
    /// [`crate::gguf::parse_header`] interpreta ([`SUPPORTED_VERSIONS`]).
    UnsupportedVersion {
        /// A versão declarada no ficheiro.
        version: u32,
    },
    /// Um campo declarado termina além dos bytes que o ficheiro tem.
    ///
    /// O intervalo `offset..end` é o do **que não caberia**, que nem sempre é um
    /// campo: quando [`Field::Alignment`] é o campo citado, o intervalo é o do
    /// *padding* que o alinhamento declarado exige, e não o do campo
    /// `general.alignment` — o campo cabe, a secção de dados é que não.
    Truncated {
        /// O campo cujo fim caiu fora do buffer, ou [`Field::Alignment`] quando
        /// o que não cabe é a secção de dados que o alinhamento localizou.
        field: Field,
        /// O deslocamento onde o intervalo começa.
        offset: u64,
        /// O deslocamento onde o intervalo terminaria.
        end: u64,
        /// Quantos bytes o ficheiro realmente tem.
        available: u64,
    },
    /// O fim de um campo transbordou a aritmética antes de poder ser comparado
    /// com o tamanho do ficheiro.
    ArithmeticOverflow {
        /// O campo cujo fim não pôde ser calculado.
        field: Field,
        /// O deslocamento onde o cálculo começou.
        offset: u64,
    },
    /// Um valor declarado excedeu o limite correspondente.
    LimitExceeded {
        /// O limite excedido.
        limit: Limit,
        /// O valor que o ficheiro declarou.
        found: u64,
        /// O máximo permitido.
        max: u64,
        /// O deslocamento do valor declarado.
        offset: u64,
    },
    /// O tipo declarado de um valor de metadados não existe em GGUF (0 a 12).
    UnknownMetadataValueType {
        /// O tipo declarado.
        value_type: u32,
        /// O deslocamento do campo de tipo.
        offset: u64,
    },
    /// Uma contagem declarada não cabe nos bytes que o ficheiro tem.
    ///
    /// É a recusa que fecha a classe do CVE-2026-5757 e do CVE-2026-7482: a
    /// contagem é confrontada com o tamanho real **antes** de qualquer iteração
    /// sobre ela.
    DeclaredCountExceedsFile {
        /// O campo que declara a contagem.
        field: Field,
        /// A contagem declarada.
        declared: u64,
        /// O mínimo de bytes que essa contagem exige.
        minimum_bytes: u64,
        /// Os bytes que o ficheiro tem depois do cabeçalho.
        available_bytes: u64,
        /// O deslocamento do campo que declara a contagem.
        offset: u64,
    },
    /// Um contador é negativo no fio.
    ///
    /// O campo é um `i64` no `llama.cpp` de referência, e um negativo não é um
    /// `u64` enorme a reinterpretar: é uma contagem impossível.
    NegativeCount {
        /// O contador negativo.
        field: Field,
    },
    /// `general.alignment` declarado como zero.
    ///
    /// Um alinhamento zero não é um alinhamento, e calcular um resto por ele
    /// seria um pânico — a mesma razão pela qual a caminhada não divide por um
    /// valor que não verificou.
    ZeroAlignment {
        /// O deslocamento do campo declarado.
        offset: u64,
    },
    /// `general.alignment` declarado com um tipo que não é inteiro.
    ///
    /// Sem saber o alinhamento não se sabe onde a secção de dados começa, e
    /// adivinhar 32 seria medir contra uma estrutura que o ficheiro não declara.
    AlignmentNotAnInteger {
        /// O tipo declarado.
        value_type: u32,
        /// O deslocamento do campo de tipo.
        offset: u64,
    },
}

impl fmt::Display for Rejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadMagic { found } => write!(
                f,
                "magic inválido: esperado `GGUF`, encontrado {found}"
            ),
            Self::UnsupportedVersion { version } => write!(
                f,
                "versão {version} não interpretada: o Gate 0 lê as versões {SUPPORTED_VERSIONS:?}"
            ),
            Self::Truncated {
                field,
                offset,
                end,
                available,
            } => write!(
                f,
                "truncado: `{}` começa em {offset} e termina em {end}, e o ficheiro tem {available} byte(s)",
                field.label()
            ),
            Self::ArithmeticOverflow { field, offset } => write!(
                f,
                "overflow aritmético ao calcular o fim de `{}` a partir de {offset}",
                field.label()
            ),
            Self::LimitExceeded {
                limit,
                found,
                max,
                offset,
            } => write!(
                f,
                "limite `{}` excedido: {found} > {max} (no deslocamento {offset})",
                limit.label()
            ),
            Self::UnknownMetadataValueType { value_type, offset } => write!(
                f,
                "tipo de valor de metadados desconhecido: {value_type} (0 a 12 são os definidos em GGUF), no deslocamento {offset}"
            ),
            Self::DeclaredCountExceedsFile {
                field,
                declared,
                minimum_bytes,
                available_bytes,
                offset,
            } => write!(
                f,
                "`{}` declara {declared}, que exige no mínimo {minimum_bytes} byte(s), e o ficheiro só tem {available_bytes} byte(s) depois do cabeçalho (campo no deslocamento {offset})",
                field.label()
            ),
            Self::NegativeCount { field } => write!(
                f,
                "`{}` é negativo no ficheiro: o campo é um i64, e um negativo não é um u64 a reinterpretar",
                field.label()
            ),
            Self::ZeroAlignment { offset } => write!(
                f,
                "`general.alignment` declarado como zero (deslocamento {offset}): um alinhamento zero não localiza a secção de dados"
            ),
            Self::AlignmentNotAnInteger { value_type, offset } => write!(
                f,
                "`general.alignment` declarado com o tipo {value_type}, que não é inteiro (deslocamento {offset})"
            ),
        }
    }
}

/// O veredito do Gate 0 sobre os bytes de um ficheiro.
///
/// Os contadores do percurso são reportados **mesmo quando a caminhada é
/// recusada**: um ficheiro recusado no par 900 de 1 000 reporta 899 pares
/// percorridos e os orçamentos consumidos até ali, em vez de zeros. É a mesma
/// escolha de [`crate::gguf::GgufHeaderReport`], que reporta os campos que os
/// bytes presentes permitiram ler.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SanitizeReport {
    /// `true` somente se todo o percurso passou: cabeçalho, secção de
    /// pares chave-valor, descritores de tensor, alinhamento e limites.
    pub ok: bool,
    /// Quantos bytes o chamador entregou.
    pub available_bytes: usize,
    /// A versão interpretada, se o cabeçalho interpretou.
    pub version: Option<u32>,
    /// O número de pares chave-valor que o cabeçalho declara.
    pub declared_kv_count: Option<u64>,
    /// O número de tensores que o cabeçalho declara.
    pub declared_tensor_count: Option<u64>,
    /// Quantos pares chave-valor o percurso atravessou de facto.
    pub kv_pairs_walked: u64,
    /// Quantos descritores de tensor o percurso atravessou de facto.
    pub tensors_walked: u64,
    /// O tamanho, em bytes, da secção de metadados e descritores lida do
    /// ficheiro — do fim do cabeçalho até ao fim do último descritor.
    pub metadata_bytes: u64,
    /// O orçamento descodificado comprometido, em bytes — o que um leitor que
    /// materializasse estes metadados alocaria, pelo piso de
    /// [`DECODED_SCALAR_WIDTH`].
    pub decoded_metadata_bytes: u64,
    /// A maior profundidade de arrays aberta durante o percurso.
    pub max_array_nesting_seen: u64,
    /// O alinhamento declarado por `general.alignment`, quando o ficheiro o
    /// declara.
    pub alignment: Option<u64>,
    /// Onde a secção de dados começa: o fim dos descritores mais o padding de
    /// alinhamento. `None` quando a caminhada não chegou lá.
    pub data_offset: Option<u64>,
    /// Quantos bytes de secção de dados o ficheiro tem a partir de
    /// [`Self::data_offset`].
    pub data_bytes_available: Option<u64>,
    /// O maior deslocamento de tensor declarado, relativo ao início da secção
    /// de dados.
    pub max_tensor_offset: Option<u64>,
    /// A recusa tipada, ou `None` se passou.
    pub rejection: Option<Rejection>,
    /// A mesma recusa em forma legível, ou `None` se passou.
    ///
    /// Existe ao lado de [`Self::rejection`] para que quem não queira combinar
    /// com o tipo tenha a frase pronta, como em [`crate::gguf`].
    pub error: Option<String>,
}

/// Sanita os metadados de `bytes` com os limites por omissão
/// ([`SanitizeLimits::default`]).
///
/// Nunca devolve `Err`: um ficheiro truncado, um magic errado e uma contagem
/// impossível são **resultados**, com a razão em [`SanitizeReport::rejection`].
/// Não aloca a partir de nada que o ficheiro declare e não lê um byte além de
/// `bytes`.
pub fn sanitize_gguf(bytes: &[u8]) -> SanitizeReport {
    sanitize_gguf_with_limits(bytes, &SanitizeLimits::default())
}

/// Sanita os metadados de `bytes` com limites explícitos.
///
/// Os limites são um argumento e não uma constante porque quem chama sabe mais
/// do que o Gate 0 sobre o ficheiro que vai verificar: um conversor que aceite
/// strings longas pode subir [`SanitizeLimits::max_string_len`] e continua a ter
/// a verificação estrutural toda. Baixá-los é a razão principal de existir: um
/// serviço que só aceite modelos pequenos aperta os orçamentos sem reescrever
/// nada.
pub fn sanitize_gguf_with_limits(bytes: &[u8], limits: &SanitizeLimits) -> SanitizeReport {
    let mut sanitizer = Sanitizer::new(bytes, limits);
    match sanitizer.run() {
        Ok(()) => sanitizer.into_report(None),
        Err(rejection) => sanitizer.into_report(Some(rejection)),
    }
}

/// O estado do percurso sobre os bytes crus.
///
/// Nenhum campo é uma coleção do tamanho do ficheiro: os contadores são
/// escalares e a secção é percorrida sem ser guardada.
struct Sanitizer<'a> {
    /// Os bytes crus, na íntegra.
    bytes: &'a [u8],
    /// Os limites com que o percurso corre.
    limits: &'a SanitizeLimits,
    /// O deslocamento do próximo byte a consumir.
    pos: usize,
    /// A versão declarada, quando o cabeçalho interpretou.
    version: Option<u32>,
    /// O número de pares chave-valor declarado.
    declared_kv_count: Option<u64>,
    /// O número de tensores declarado.
    declared_tensor_count: Option<u64>,
    /// Quantos pares chave-valor foram percorridos.
    kv_pairs_walked: u64,
    /// Quantos descritores de tensor foram percorridos.
    tensors_walked: u64,
    /// O orçamento descodificado já comprometido.
    decoded: u64,
    /// A maior profundidade de arrays aberta.
    nesting_seen: u64,
    /// O alinhamento declarado por `general.alignment`.
    alignment: Option<u64>,
    /// O maior deslocamento de tensor declarado e o deslocamento do campo que o
    /// declarou, enquanto algum tensor tiver sido percorrido.
    max_tensor_offset: Option<(u64, u64)>,
    /// Onde a secção de dados começa.
    data_offset: Option<u64>,
    /// Quantos bytes de secção de dados existem.
    data_bytes_available: Option<u64>,
}

impl<'a> Sanitizer<'a> {
    /// Um percurso novo, posicionado no fim do cabeçalho.
    fn new(bytes: &'a [u8], limits: &'a SanitizeLimits) -> Self {
        Self {
            bytes,
            limits,
            pos: HEADER_LEN,
            version: None,
            declared_kv_count: None,
            declared_tensor_count: None,
            kv_pairs_walked: 0,
            tensors_walked: 0,
            decoded: 0,
            nesting_seen: 0,
            alignment: None,
            max_tensor_offset: None,
            data_offset: None,
            data_bytes_available: None,
        }
    }

    /// O relatório, com a recusa quando houve uma.
    fn into_report(self, rejection: Option<Rejection>) -> SanitizeReport {
        let error = rejection.as_ref().map(ToString::to_string);
        SanitizeReport {
            ok: rejection.is_none(),
            available_bytes: self.bytes.len(),
            version: self.version,
            declared_kv_count: self.declared_kv_count,
            declared_tensor_count: self.declared_tensor_count,
            kv_pairs_walked: self.kv_pairs_walked,
            tensors_walked: self.tensors_walked,
            metadata_bytes: (self.pos.saturating_sub(HEADER_LEN)) as u64,
            decoded_metadata_bytes: self.decoded,
            max_array_nesting_seen: self.nesting_seen,
            alignment: self.alignment,
            data_offset: self.data_offset,
            data_bytes_available: self.data_bytes_available,
            max_tensor_offset: self.max_tensor_offset.map(|(offset, _)| offset),
            rejection,
            error,
        }
    }

    /// O percurso inteiro, na ordem em que as verificações têm de acontecer.
    fn run(&mut self) -> Result<(), Rejection> {
        let header = parse_header(self.bytes);
        if !header.ok {
            return Err(self.reject_header(&header));
        }

        // `parse_header` só devolve `ok: true` com os três campos presentes; o
        // `else` é uma guarda de tipo, não um caminho alcançável.
        let (Some(version), Some(kv_count), Some(tensor_count)) =
            (header.version, header.metadata_kv_count, header.tensor_count)
        else {
            return Err(self.reject_header(&header));
        };
        self.version = Some(version);
        self.declared_kv_count = Some(kv_count);
        self.declared_tensor_count = Some(tensor_count);

        // As contagens declaradas primeiro (CVE-2026-65315): nada é percorrido
        // antes de elas caberem nos limites.
        self.check_limit(Limit::KvPairs, kv_count, self.limits.max_kv_pairs, HEADER_LEN as u64)?;
        self.check_limit(
            Limit::TensorCount,
            tensor_count,
            self.limits.max_tensor_count,
            HEADER_LEN as u64,
        )?;

        // E depois o confronto com o tamanho real do ficheiro (CVE-2026-5757,
        // CVE-2026-7482): uma contagem que não cabe nos bytes presentes é
        // recusada sem iterar sobre ela. O mínimo por par/descritor é um piso —
        // a caminhada confirma o resto, elemento a elemento.
        let available = self.bytes.len().saturating_sub(HEADER_LEN) as u64;
        let kv_minimum = kv_count.checked_mul(MIN_KV_BYTES).ok_or(
            Rejection::ArithmeticOverflow {
                field: Field::KvCount,
                offset: 16,
            },
        )?;
        if kv_minimum > available {
            return Err(Rejection::DeclaredCountExceedsFile {
                field: Field::KvCount,
                declared: kv_count,
                minimum_bytes: kv_minimum,
                available_bytes: available,
                offset: 16,
            });
        }
        let tensor_minimum = tensor_count.checked_mul(MIN_TENSOR_BYTES).ok_or(
            Rejection::ArithmeticOverflow {
                field: Field::TensorCount,
                offset: 8,
            },
        )?;
        if tensor_minimum > available {
            return Err(Rejection::DeclaredCountExceedsFile {
                field: Field::TensorCount,
                declared: tensor_count,
                minimum_bytes: tensor_minimum,
                available_bytes: available,
                offset: 8,
            });
        }

        self.walk_metadata(kv_count)?;
        self.walk_tensors(tensor_count)?;
        self.finish_layout(tensor_count)
    }

    /// Traduz o relatório de [`parse_header`] numa recusa tipada, sem ler as
    /// frases dele: os factos que o relatório expõe bastam para distinguir os
    /// modos de falha.
    fn reject_header(&self, header: &crate::gguf::GgufHeaderReport) -> Rejection {
        let available = self.bytes.len();

        // Menos de quatro bytes não é um magic errado: é um ficheiro truncado, e
        // não há quatro bytes para citar.
        if available < GGUF_MAGIC.len() {
            return Rejection::Truncated {
                field: Field::Magic,
                offset: 0,
                end: GGUF_MAGIC.len() as u64,
                available: available as u64,
            };
        }
        if !header.magic_ok {
            // A partir daqui os quatro bytes existem: o `unwrap_or` nunca corre.
            let found = self
                .bytes
                .get(..GGUF_MAGIC.len())
                .map(encode_hex)
                .unwrap_or_else(|| format!("{available} byte(s)"));
            return Rejection::BadMagic { found };
        }
        let Some(version) = header.version else {
            return Rejection::Truncated {
                field: Field::Version,
                offset: 4,
                end: 8,
                available: available as u64,
            };
        };
        if !SUPPORTED_VERSIONS.contains(&version) {
            return Rejection::UnsupportedVersion { version };
        }
        // Magic e versão válidos: só um contador pode ter falhado, e o `None` só
        // vem de um valor negativo quando os 24 bytes estão todos presentes.
        let field = if header.tensor_count.is_none() {
            Field::TensorCount
        } else {
            Field::KvCount
        };
        if available >= HEADER_LEN {
            return Rejection::NegativeCount { field };
        }
        Rejection::Truncated {
            field,
            offset: 0,
            end: HEADER_LEN as u64,
            available: available as u64,
        }
    }

    /// Recusa se `found` exceder `max`. É a única forma de aplicar um limite.
    fn check_limit(
        &self,
        limit: Limit,
        found: u64,
        max: u64,
        offset: u64,
    ) -> Result<(), Rejection> {
        if found > max {
            return Err(Rejection::LimitExceeded {
                limit,
                found,
                max,
                offset,
            });
        }
        Ok(())
    }

    /// Soma `bytes` ao orçamento descodificado e confere o orçamento.
    ///
    /// A soma é verificada: um array de comprimento suficiente para transbordar
    /// o `u64` do orçamento é recusado como overflow em vez de dar a volta e
    /// passar, que é exatamente o que o CVE-2026-86289 fazia.
    fn add_decoded(&mut self, bytes: u64, offset: u64) -> Result<(), Rejection> {
        self.decoded = self
            .decoded
            .checked_add(bytes)
            .ok_or(Rejection::ArithmeticOverflow {
                field: Field::Value,
                offset,
            })?;
        self.check_limit(
            Limit::DecodedMetadataSize,
            self.decoded,
            self.limits.max_decoded_metadata_size,
            offset,
        )
    }

    /// Consome `len` bytes e devolve-os, ou recusa se o fim cair fora do buffer.
    ///
    /// Toda a aritmética de deslocamento passa aqui: o `try_from` cobre uma
    /// plataforma de 32 bits (onde um `u64` grande não cabe num `usize`) e o
    /// `checked_add` cobre o transbordo. O `get` garante que o deslocamento
    /// calculado está dentro dos bytes — não há indexação direta neste módulo.
    fn take(&mut self, len: u64, field: Field) -> Result<&'a [u8], Rejection> {
        let start = self.pos;
        let len = usize::try_from(len).map_err(|_| Rejection::ArithmeticOverflow {
            field,
            offset: start as u64,
        })?;
        let end = start.checked_add(len).ok_or(Rejection::ArithmeticOverflow {
            field,
            offset: start as u64,
        })?;
        let slice = self.bytes.get(start..end).ok_or(Rejection::Truncated {
            field,
            offset: start as u64,
            end: end as u64,
            available: self.bytes.len() as u64,
        })?;
        self.pos = end;
        Ok(slice)
    }

    /// Lê um inteiro little-endian de `N` bytes.
    ///
    /// O `copy_from_slice` não pode entrar em pânico: `take(N)` devolve uma
    /// slice de exatamente `N` bytes, porque o fim que ele validou é
    /// `início + N`.
    fn read_array<const N: usize>(&mut self, field: Field) -> Result<[u8; N], Rejection> {
        let src = self.take(N as u64, field)?;
        let mut buffer = [0u8; N];
        buffer.copy_from_slice(src);
        Ok(buffer)
    }

    /// Lê um `u64` little-endian.
    fn read_u64(&mut self, field: Field) -> Result<u64, Rejection> {
        Ok(u64::from_le_bytes(self.read_array::<8>(field)?))
    }

    /// Confere o orçamento da secção lida do ficheiro no ponto atual.
    fn check_metadata_budget(&self, offset: u64) -> Result<(), Rejection> {
        let consumed = self.pos.saturating_sub(HEADER_LEN) as u64;
        self.check_limit(
            Limit::MetadataSize,
            consumed,
            self.limits.max_metadata_size,
            offset,
        )
    }

    /// Percorre os pares chave-valor declarados.
    fn walk_metadata(&mut self, kv_count: u64) -> Result<(), Rejection> {
        for _ in 0..kv_count {
            let entry_offset = self.pos as u64;
            // O orçamento da secção é conferido no início de cada par: um
            // ficheiro cuja secção de metadados já passou do limite é recusado
            // sem percorrer o resto dela.
            self.check_metadata_budget(entry_offset)?;

            let key_len = self.read_u64(Field::StringLength)?;
            self.check_limit(
                Limit::StringLength,
                key_len,
                self.limits.max_string_len,
                entry_offset,
            )?;
            let key = self.take(key_len, Field::StringBytes)?;
            let key_is_alignment = key == GENERAL_ALIGNMENT;
            let key_len_cost = key_len
                .checked_add(STRING_LEN_PREFIX)
                .ok_or(Rejection::ArithmeticOverflow {
                    field: Field::StringLength,
                    offset: entry_offset,
                })?;
            self.add_decoded(key_len_cost, entry_offset)?;

            // O tipo do valor vem depois da chave e antes do valor; o
            // deslocamento reportado numa recusa do valor é o do valor, não o do
            // tipo.
            let value_type = u32::from_le_bytes(self.read_array::<4>(Field::ValueType)?);
            let value_offset = self.pos as u64;
            self.walk_value(value_type, 1, value_offset, key_is_alignment)?;

            self.kv_pairs_walked += 1;
        }
        Ok(())
    }

    /// Percorre um valor de metadados e compromete o orçamento descodificado.
    ///
    /// `depth` é a profundidade **deste** valor: 1 para um valor de topo, e mais
    /// um por cada array que o contenha. `alignment_key` diz se este valor é o
    /// de `general.alignment`, que não é um valor como os outros — é o que
    /// localiza a secção de dados.
    fn walk_value(
        &mut self,
        value_type: u32,
        depth: u64,
        offset: u64,
        alignment_key: bool,
    ) -> Result<(), Rejection> {
        if alignment_key {
            return self.read_alignment(value_type, offset);
        }

        if let Some(width) = scalar_width(value_type) {
            self.take(width, Field::Value)?;
            return self.add_decoded(DECODED_SCALAR_WIDTH, offset);
        }

        match value_type {
            // STRING
            8 => {
                let length = self.read_u64(Field::StringLength)?;
                self.check_limit(
                    Limit::StringLength,
                    length,
                    self.limits.max_string_len,
                    offset,
                )?;
                self.take(length, Field::StringBytes)?;
                let cost = length
                    .checked_add(DECODED_SCALAR_WIDTH)
                    .ok_or(Rejection::ArithmeticOverflow {
                        field: Field::StringLength,
                        offset,
                    })?;
                self.add_decoded(cost, offset)
            }
            // ARRAY
            9 => self.walk_array(depth, offset),
            other => Err(Rejection::UnknownMetadataValueType {
                value_type: other,
                offset,
            }),
        }
    }

    /// Percorre um array de metadados: o cabeçalho, o limite de elementos e os
    /// elementos, cujo custo descodificado é o que o orçamento descodificado
    /// existe para limitar.
    fn walk_array(&mut self, depth: u64, offset: u64) -> Result<(), Rejection> {
        // O limite de aninhamento é conferido antes de abrir mais um nível: sem
        // isto, um ficheiro com 12 bytes por nível esgotaria a pilha.
        self.check_limit(
            Limit::ArrayNesting,
            depth,
            self.limits.max_array_nesting,
            offset,
        )?;
        self.nesting_seen = self.nesting_seen.max(depth);

        let header_offset = self.pos as u64;
        let element_type = u32::from_le_bytes(self.read_array::<4>(Field::ArrayHeader)?);
        let count = u64::from_le_bytes(self.read_array::<8>(Field::ArrayHeader)?);
        self.check_limit(
            Limit::ArrayElements,
            count,
            self.limits.max_array_elements,
            header_offset,
        )?;
        self.add_decoded(ARRAY_HEADER_LEN, header_offset)?;

        if let Some(width) = scalar_width(element_type) {
            // Um array de escalares de largura fixa atravessa-se com uma
            // multiplicação verificada, sem iterar elemento a elemento: são
            // `count` elementos e cada um custa o mesmo. A multiplicação é
            // verificada porque `count` chega de um `u64` do ficheiro.
            let total = count.checked_mul(width).ok_or(Rejection::ArithmeticOverflow {
                field: Field::ArrayElement,
                offset: header_offset,
            })?;
            self.take(total, Field::ArrayElement)?;
            let decoded = count
                .checked_mul(DECODED_SCALAR_WIDTH)
                .ok_or(Rejection::ArithmeticOverflow {
                    field: Field::ArrayElement,
                    offset: header_offset,
                })?;
            return self.add_decoded(decoded, header_offset);
        }

        match element_type {
            // Um array de strings tem de ser percorrido: cada elemento traz o
            // seu próprio comprimento, e é a soma deles que o orçamento mede.
            8 => {
                for _ in 0..count {
                    let element_offset = self.pos as u64;
                    let length = self.read_u64(Field::StringLength)?;
                    self.check_limit(
                        Limit::StringLength,
                        length,
                        self.limits.max_string_len,
                        element_offset,
                    )?;
                    self.take(length, Field::StringBytes)?;
                    let cost = length
                        .checked_add(DECODED_SCALAR_WIDTH)
                        .ok_or(Rejection::ArithmeticOverflow {
                            field: Field::StringLength,
                            offset: element_offset,
                        })?;
                    self.add_decoded(cost, element_offset)?;
                }
                Ok(())
            }
            // E um array de arrays é recursivo, com a profundidade a subir um
            // por nível até o limite de aninhamento o travar.
            9 => {
                for _ in 0..count {
                    let element_offset = self.pos as u64;
                    let nested = depth
                        .checked_add(1)
                        .ok_or(Rejection::ArithmeticOverflow {
                            field: Field::ArrayElement,
                            offset: element_offset,
                        })?;
                    self.walk_value(9, nested, element_offset, false)?;
                }
                Ok(())
            }
            other => Err(Rejection::UnknownMetadataValueType {
                value_type: other,
                offset: header_offset,
            }),
        }
    }

    /// Lê o valor de `general.alignment`.
    ///
    /// Só inteiros servem, e zero não serve: sem um alinhamento não negativo
    /// não há um resto que se possa calcular, e calcular `% 0` seria um pânico.
    fn read_alignment(&mut self, value_type: u32, offset: u64) -> Result<(), Rejection> {
        let alignment = match value_type {
            4 => u32::from_le_bytes(self.read_array::<4>(Field::Alignment)?) as u64,
            10 => u64::from_le_bytes(self.read_array::<8>(Field::Alignment)?),
            other => {
                return Err(Rejection::AlignmentNotAnInteger {
                    value_type: other,
                    offset,
                })
            }
        };
        if alignment == 0 {
            return Err(Rejection::ZeroAlignment { offset });
        }
        self.alignment = Some(alignment);
        self.add_decoded(DECODED_SCALAR_WIDTH, offset)
    }

    /// Percorre os descritores de tensor declarados.
    fn walk_tensors(&mut self, tensor_count: u64) -> Result<(), Rejection> {
        for _ in 0..tensor_count {
            let entry_offset = self.pos as u64;
            self.check_metadata_budget(entry_offset)?;

            let name_len = self.read_u64(Field::StringLength)?;
            self.check_limit(
                Limit::StringLength,
                name_len,
                self.limits.max_string_len,
                entry_offset,
            )?;
            self.take(name_len, Field::StringBytes)?;
            let name_cost = name_len
                .checked_add(STRING_LEN_PREFIX)
                .ok_or(Rejection::ArithmeticOverflow {
                    field: Field::StringLength,
                    offset: entry_offset,
                })?;
            self.add_decoded(name_cost, entry_offset)?;

            let dims_offset = self.pos as u64;
            let dims = u32::from_le_bytes(self.read_array::<4>(Field::TensorDims)?) as u64;
            // CVE-2026-53923: o número de dimensões é limitado antes de ser
            // usado como contagem de um laço.
            self.check_limit(
                Limit::TensorDims,
                dims,
                self.limits.max_tensor_dims,
                dims_offset,
            )?;
            for _ in 0..dims {
                let dimension_offset = self.pos as u64;
                let dimension = u64::from_le_bytes(self.read_array::<8>(Field::TensorDimension)?);
                // CVE-2026-53923, a outra metade: uma dimensão que não cabe num
                // `u32` é a que um kernel que a receba em `u32` trunca.
                self.check_limit(
                    Limit::TensorDimValue,
                    dimension,
                    self.limits.max_tensor_dim_value,
                    dimension_offset,
                )?;
                self.add_decoded(DECODED_SCALAR_WIDTH, dimension_offset)?;
            }

            let _tensor_type = self.read_array::<4>(Field::TensorType)?;

            let offset_field = self.pos as u64;
            let tensor_offset = self.read_u64(Field::TensorOffset)?;
            if self
                .max_tensor_offset
                .is_none_or(|(previous, _)| tensor_offset >= previous)
            {
                self.max_tensor_offset = Some((tensor_offset, offset_field));
            }

            self.tensors_walked += 1;
        }
        Ok(())
    }

    /// Fecha o percurso: o orçamento da secção lida, o padding de alinhamento, o
    /// início da secção de dados e os deslocamentos declarados dos tensores.
    fn finish_layout(&mut self, tensor_count: u64) -> Result<(), Rejection> {
        let descriptors_end = self.pos;
        let metadata_bytes = descriptors_end.saturating_sub(HEADER_LEN) as u64;
        self.check_limit(
            Limit::MetadataSize,
            metadata_bytes,
            self.limits.max_metadata_size,
            HEADER_LEN as u64,
        )?;

        // Sem tensores não há secção de dados e não há padding a exigir. É a
        // regra que permite que a fixture real — 24 bytes, zero tensores, zero
        // pares — passe sem oito bytes de alinhamento que não existem.
        let data_offset = if tensor_count == 0 {
            descriptors_end
        } else {
            let alignment = self.alignment.unwrap_or(DEFAULT_ALIGNMENT);
            let remainder = (descriptors_end as u64) % alignment;
            let padding = (alignment - remainder) % alignment;
            let aligned = (descriptors_end as u64)
                .checked_add(padding)
                .ok_or(Rejection::ArithmeticOverflow {
                    field: Field::Alignment,
                    offset: descriptors_end as u64,
                })?;
            usize::try_from(aligned).map_err(|_| Rejection::ArithmeticOverflow {
                field: Field::Alignment,
                offset: descriptors_end as u64,
            })?
        };

        if data_offset > self.bytes.len() {
            return Err(Rejection::Truncated {
                field: Field::Alignment,
                offset: descriptors_end as u64,
                end: data_offset as u64,
                available: self.bytes.len() as u64,
            });
        }
        self.data_offset = Some(data_offset as u64);
        self.data_bytes_available = Some((self.bytes.len() - data_offset) as u64);

        if tensor_count > 0 {
            let (declared_offset, declared_field) = self.max_tensor_offset.unwrap_or((0, 0));
            let end = (data_offset as u64)
                .checked_add(declared_offset)
                .ok_or(Rejection::ArithmeticOverflow {
                    field: Field::TensorOffset,
                    offset: declared_field,
                })?;
            if end > self.bytes.len() as u64 {
                return Err(Rejection::Truncated {
                    field: Field::TensorOffset,
                    offset: declared_field,
                    end,
                    available: self.bytes.len() as u64,
                });
            }
        }
        Ok(())
    }
}

/// A largura em bytes de um tipo escalar de metadados GGUF, ou `None` se o tipo
/// não for escalar de largura fixa (string, array, ou um tipo que não existe).
///
/// Os tipos são os do GGUF: 0 `UINT8`, 1 `INT8`, 2 `UINT16`, 3 `INT16`,
/// 4 `UINT32`, 5 `INT32`, 6 `FLOAT32`, 7 `BOOL`, 8 `STRING`, 9 `ARRAY`,
/// 10 `UINT64`, 11 `INT64`, 12 `FLOAT64`.
fn scalar_width(value_type: u32) -> Option<u64> {
    match value_type {
        0 | 1 | 7 => Some(1),
        2 | 3 => Some(2),
        4 | 5 | 6 => Some(4),
        10 | 11 | 12 => Some(8),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fixture real do crate: 24 bytes, versão 3, zero tensores e zero pares
    /// chave-valor. Embutida com `include_bytes!` e não lida com `fs::read`: um
    /// teste de unidade não deve depender do diretório de trabalho.
    const FIXTURE: &[u8] = include_bytes!("../fixtures/arkhe.gguf");

    /// O cabeçalho de 24 bytes de uma versão, com as duas contagens declaradas.
    fn header(version: u32, tensor_count: u64, kv_count: u64) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(HEADER_LEN);
        bytes.extend_from_slice(&GGUF_MAGIC);
        bytes.extend_from_slice(&version.to_le_bytes());
        bytes.extend_from_slice(&tensor_count.to_le_bytes());
        bytes.extend_from_slice(&kv_count.to_le_bytes());
        bytes
    }

    /// Uma string GGUF: comprimento `u64` little-endian mais os bytes.
    fn string(bytes: &mut Vec<u8>, value: &[u8]) {
        bytes.extend_from_slice(&(value.len() as u64).to_le_bytes());
        bytes.extend_from_slice(value);
    }

    /// O corpo de um par chave-valor, sem o cabeçalho do ficheiro.
    fn kv_body(key: &str, value_type: u32, value: &[u8]) -> Vec<u8> {
        let mut body = Vec::new();
        string(&mut body, key.as_bytes());
        body.extend_from_slice(&value_type.to_le_bytes());
        body.extend_from_slice(value);
        body
    }

    /// Um par chave-valor escalar `U32`.
    fn kv_u32(key: &str, value: u32) -> Vec<u8> {
        kv_body(key, 4, &value.to_le_bytes())
    }

    /// Um par chave-valor com valor string.
    fn kv_string(key: &str, value: &str) -> Vec<u8> {
        let mut body = Vec::new();
        string(&mut body, value.as_bytes());
        kv_body(key, 8, &body)
    }

    /// Um par chave-valor cujo valor declara um array de `count` elementos **sem
    /// os trazer**: é o que um ficheiro malformado faz, e é a declaração que o
    /// Gate 0 tem de confrontar com os bytes reais.
    fn kv_array_declaring(key: &str, element_type: u32, count: u64) -> Vec<u8> {
        let mut value = Vec::new();
        value.extend_from_slice(&element_type.to_le_bytes());
        value.extend_from_slice(&count.to_le_bytes());
        kv_body(key, 9, &value)
    }

    /// Um par chave-valor com um array de `count` bytes `U8` **presentes**.
    fn kv_array_of_u8(key: &str, count: u64) -> Vec<u8> {
        let mut value = Vec::new();
        value.extend_from_slice(&0u32.to_le_bytes()); // UINT8
        value.extend_from_slice(&count.to_le_bytes());
        value.resize(value.len() + count as usize, 0x11);
        kv_body(key, 9, &value)
    }

    /// O valor de `arrays` arrays aninhados, com um array de um `U8` no fundo.
    ///
    /// `arrays` conta todos, incluindo o mais interior: `nested_array_value(1)`
    /// é um `ARRAY(UINT8, 1)`, e `nested_array_value(3)` é
    /// `ARRAY(ARRAY(ARRAY(UINT8, 1), 1), 1)`.
    fn nested_array_value(arrays: u64) -> Vec<u8> {
        let mut value = Vec::new();
        for _ in 0..arrays.saturating_sub(1) {
            value.extend_from_slice(&9u32.to_le_bytes()); // ARRAY
            value.extend_from_slice(&1u64.to_le_bytes());
        }
        value.extend_from_slice(&0u32.to_le_bytes()); // UINT8
        value.extend_from_slice(&1u64.to_le_bytes());
        value.push(0x11);
        value
    }

    /// Um descritor de tensor, byte a byte.
    fn tensor(name: &str, dims: &[u64], ggml_type: u32, offset: u64) -> Vec<u8> {
        let mut bytes = Vec::new();
        string(&mut bytes, name.as_bytes());
        bytes.extend_from_slice(&(dims.len() as u32).to_le_bytes());
        for dim in dims {
            bytes.extend_from_slice(&dim.to_le_bytes());
        }
        bytes.extend_from_slice(&ggml_type.to_le_bytes());
        bytes.extend_from_slice(&offset.to_le_bytes());
        bytes
    }

    /// Um ficheiro cujas contagens do cabeçalho são calculadas do que o corpo
    /// traz — o caso bem formado.
    fn file(version: u32, kvs: &[Vec<u8>], tensors: &[Vec<u8>]) -> Vec<u8> {
        let mut bytes = header(version, tensors.len() as u64, kvs.len() as u64);
        for kv in kvs {
            bytes.extend_from_slice(kv);
        }
        for tensor in tensors {
            bytes.extend_from_slice(tensor);
        }
        bytes
    }

    /// Um ficheiro com um corpo arbitrário e as contagens declaradas à mão — o
    /// caso em que a declaração e o conteúdo divergem.
    fn file_declaring(version: u32, tensor_count: u64, kv_count: u64, body: &[u8]) -> Vec<u8> {
        let mut bytes = header(version, tensor_count, kv_count);
        bytes.extend_from_slice(body);
        bytes
    }

    /// Um ficheiro com um tensor, o padding de alinhamento a 32 e `data_bytes`
    /// bytes de secção de dados.
    fn file_with_one_tensor(name: &str, dims: &[u64], offset: u64, data_bytes: usize) -> Vec<u8> {
        let mut bytes = file(3, &[], &[tensor(name, dims, 0, offset)]);
        let padding = (32 - bytes.len() % 32) % 32;
        bytes.resize(bytes.len() + padding, 0);
        bytes.resize(bytes.len() + data_bytes, 0x22);
        bytes
    }

    /// A recusa de um relatório, exigindo que ele tenha sido recusado.
    fn rejection(report: &SanitizeReport) -> Rejection {
        assert!(!report.ok, "esperava uma recusa, veio ok: {report:?}");
        report.rejection.clone().expect("uma recusa tem de ter causa")
    }

    // --- o caminho válido -------------------------------------------------

    #[test]
    fn the_real_fixture_passes() {
        let report = sanitize_gguf(FIXTURE);

        assert!(report.ok, "erro: {:?}", report.error);
        assert_eq!(report.available_bytes, 24);
        assert_eq!(report.version, Some(3));
        assert_eq!(report.declared_tensor_count, Some(0));
        assert_eq!(report.declared_kv_count, Some(0));
        assert_eq!(report.kv_pairs_walked, 0);
        assert_eq!(report.tensors_walked, 0);
        assert_eq!(report.metadata_bytes, 0);
        assert_eq!(report.decoded_metadata_bytes, 0);
        assert_eq!(report.max_array_nesting_seen, 0);
        assert_eq!(report.data_offset, Some(24));
        assert_eq!(report.data_bytes_available, Some(0));
        assert_eq!(report.max_tensor_offset, None);
        assert_eq!(report.rejection, None);
        assert_eq!(report.error, None);
    }

    #[test]
    fn a_well_formed_metadata_section_passes() {
        // Um par de cada forma: escalar, string, array de escalares, array de
        // strings e array de arrays — mais um descritor de tensor.
        let strings = {
            let mut value = Vec::new();
            value.extend_from_slice(&8u32.to_le_bytes()); // STRING
            value.extend_from_slice(&2u64.to_le_bytes());
            string(&mut value, b"ab");
            string(&mut value, b"cde");
            kv_body("arkhe.strings", 9, &value)
        };
        let kvs = vec![
            kv_u32("general.alignment", 32),
            kv_string("general.architecture", "arkhe"),
            kv_array_of_u8("arkhe.array", 3),
            strings,
            kv_body("arkhe.nested", 9, &nested_array_value(2)),
        ];
        let mut bytes = file(3, &kvs, &[tensor("t", &[2, 3], 0, 0)]);
        let padding = (32 - bytes.len() % 32) % 32;
        bytes.resize(bytes.len() + padding, 0);
        bytes.resize(bytes.len() + 24, 0x33);

        let report = sanitize_gguf(&bytes);

        assert!(report.ok, "erro: {:?}", report.error);
        assert_eq!(report.version, Some(3));
        assert_eq!(report.kv_pairs_walked, 5);
        assert_eq!(report.tensors_walked, 1);
        assert_eq!(report.alignment, Some(32));
        assert_eq!(report.max_array_nesting_seen, 2);
        assert_eq!(report.max_tensor_offset, Some(0));
        assert_eq!(report.data_offset, Some((bytes.len() - 24) as u64));
        assert_eq!(report.data_bytes_available, Some(24));
        // As duas contas, à mão, para que a contabilidade do percurso seja
        // visível em vez de afirmada por si mesma. Bytes do ficheiro, por
        // entrada (8 do comprimento + chave + 4 do tipo + valor):
        //   33 (`general.alignment`) + 45 (`general.architecture`) +
        //   38 (`arkhe.array`) + 58 (`arkhe.strings`) + 49 (`arkhe.nested`) +
        //   41 (o descritor do tensor) = 264.
        assert_eq!(report.metadata_bytes, 264);
        // Descodificado: cada chave/string custa 8 + o texto, cada escalar 8,
        // cada cabeçalho de array 12, cada elemento de array de `U8` 8, cada
        // dimensão 8, e os 8 + 4 + 8 do descritor (tipo do tensor e
        // deslocamento) não são materializados porque não são valores de
        // metadados: 33 + 41 + 55 + 54 + 52 + 25 = 260.
        //
        // Note-se que ele é **menor** que os bytes lidos: os dois orçamentos
        // medem coisas diferentes, e nenhum domina o outro. É por isso que
        // existem dois.
        assert_eq!(report.decoded_metadata_bytes, 260);
        assert!(report.ok);
    }

    #[test]
    fn the_default_limits_are_the_documented_table() {
        let limits = SanitizeLimits::default();

        assert_eq!(limits.max_string_len, 65_535);
        assert_eq!(limits.max_array_elements, 1_000_000);
        assert_eq!(limits.max_kv_pairs, 100_000);
        assert_eq!(limits.max_tensor_dims, 4);
        assert_eq!(limits.max_tensor_count, 1_000_000);
        assert_eq!(limits.max_metadata_size, 256 * 1024 * 1024);
        assert_eq!(limits.max_decoded_metadata_size, 256 * 1024 * 1024);
        assert_eq!(limits.max_array_nesting, 64);
        assert_eq!(limits.max_tensor_dim_value, u32::MAX as u64);
    }

    #[test]
    fn nine_megabytes_of_data_with_small_metadata_pass() {
        // O limite é da secção de metadados, não do ficheiro: um ficheiro de
        // 9 MiB com uma secção de metadados pequena tem de passar. Recusar pelo
        // tamanho do ficheiro recusaria modelos legítimos.
        let bytes = file_with_one_tensor("t", &[4], 0, 9 * 1024 * 1024);
        let report = sanitize_gguf(&bytes);

        assert!(report.ok, "erro: {:?}", report.error);
        assert_eq!(report.available_bytes, bytes.len());
        assert!(
            report.metadata_bytes < 1024,
            "a secção de metadados é pequena: {}",
            report.metadata_bytes
        );
    }

    #[test]
    fn a_limit_bites_only_when_it_is_exceeded() {
        // O mesmo ficheiro, dois limites: com o limite folgado passa, com o
        // limite apertado é recusado. É o que prova que foi o limite que recusou.
        let bytes = file(3, &[kv_string("k", "uma string de tamanho normal")], &[]);

        let generous = SanitizeLimits {
            max_string_len: 1024,
            ..SanitizeLimits::default()
        };
        assert!(sanitize_gguf_with_limits(&bytes, &generous).ok);

        let tight = SanitizeLimits {
            max_string_len: 8,
            ..SanitizeLimits::default()
        };
        let report = sanitize_gguf_with_limits(&bytes, &tight);
        assert!(matches!(
            rejection(&report),
            Rejection::LimitExceeded {
                limit: Limit::StringLength,
                found: 28,
                max: 8,
                ..
            }
        ));
    }

    #[test]
    fn zero_tensors_do_not_require_alignment_padding() {
        // Um ficheiro de pares sem tensores termina nos pares: não há secção de
        // dados, logo não há padding a exigir. É esta regra que deixa a fixture
        // de 24 bytes passar sem oito bytes de alinhamento inexistentes.
        let bytes = file(3, &[kv_u32("general.alignment", 32)], &[]);

        let report = sanitize_gguf(&bytes);

        assert!(report.ok, "erro: {:?}", report.error);
        assert_eq!(report.alignment, Some(32));
        assert_eq!(report.data_offset, Some(bytes.len() as u64));
        assert_eq!(report.data_bytes_available, Some(0));
    }

    #[test]
    fn the_report_serializes_to_the_documented_shape() {
        let report = sanitize_gguf(FIXTURE);
        let value: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&report).expect("serializa"))
                .expect("parse");

        assert_eq!(value["ok"], true);
        assert_eq!(value["version"], 3);
        assert_eq!(value["declared_kv_count"], 0);
        assert_eq!(value["kv_pairs_walked"], 0);
        assert_eq!(value["data_offset"], 24);
        assert!(value["rejection"].is_null());
        assert!(value["error"].is_null());
    }

    #[test]
    fn a_rejection_serializes_with_its_kind() {
        let bytes = file_declaring(3, 0, 99_999, &[]);
        let report = sanitize_gguf(&bytes);
        let value: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&report).expect("serializa"))
                .expect("parse");

        assert_eq!(value["ok"], false);
        assert_eq!(value["rejection"]["kind"], "declared_count_exceeds_file");
        assert_eq!(value["rejection"]["field"], "kv_count");
        assert_eq!(value["rejection"]["declared"], 99_999);
        assert!(value["error"].as_str().expect("frase").contains("99999"));
    }

    // --- negativos, um por CVE --------------------------------------------

    #[test]
    fn a_kv_string_above_max_string_len_is_rejected() {
        // CVE-2026-5757 / CVE-2026-86289: o comprimento declarado de uma string
        // é comparado com o limite antes de a string ser percorrida. O valor
        // começa no deslocamento 37 (24 do cabeçalho + 8 do comprimento da
        // chave + 1 da chave + 4 do tipo).
        let long = "x".repeat(65_536);
        let bytes = file(3, &[kv_string("k", &long)], &[]);

        let report = sanitize_gguf(&bytes);
        assert_eq!(
            rejection(&report),
            Rejection::LimitExceeded {
                limit: Limit::StringLength,
                found: 65_536,
                max: 65_535,
                offset: 37,
            },
            "a causa tem de ser o limite da string, no campo do valor"
        );

        // O mesmo ficheiro com uma string dentro do limite passa: sem isto, a
        // recusa poderia vir de outra coisa qualquer.
        let ok = file(3, &[kv_string("k", &"x".repeat(65_535))], &[]);
        assert!(sanitize_gguf(&ok).ok, "a 65 535 a string está dentro do limite");
    }

    #[test]
    fn a_key_above_max_string_len_is_rejected() {
        // A chave também é uma string, e é o primeiro campo do par: o
        // deslocamento reportado é o do início do par, 24.
        let bytes = file(3, &[kv_u32(&"k".repeat(65_536), 1)], &[]);

        let report = sanitize_gguf(&bytes);
        assert_eq!(
            rejection(&report),
            Rejection::LimitExceeded {
                limit: Limit::StringLength,
                found: 65_536,
                max: 65_535,
                offset: 24,
            }
        );
        assert!(sanitize_gguf(&file(3, &[kv_u32(&"k".repeat(1_000), 1)], &[])).ok);
    }

    #[test]
    fn an_array_above_max_array_elements_is_rejected() {
        // CVE-2026-65315: a contagem declarada de um array é limitada antes de
        // qualquer elemento ser percorrido. Note-se que o ficheiro declara
        // 1 000 001 elementos `U8` e **não traz nenhum** — o que o limita é o
        // número declarado, não os bytes presentes.
        let bytes = file(3, &[kv_array_declaring("k", 0, 1_000_001)], &[]);

        let report = sanitize_gguf(&bytes);
        assert_eq!(
            rejection(&report),
            Rejection::LimitExceeded {
                limit: Limit::ArrayElements,
                found: 1_000_001,
                max: 1_000_000,
                offset: 37,
            }
        );

        // Com 1 000 000 declarados o limite de elementos não morde, e a recusa
        // passa a ser outra — a dos bytes que faltam. É o que distingue os dois
        // limites: um conta elementos, o outro conta bytes.
        let at_limit = file(3, &[kv_array_declaring("k", 0, 1_000_000)], &[]);
        assert!(matches!(
            rejection(&sanitize_gguf(&at_limit)),
            Rejection::Truncated {
                field: Field::ArrayElement,
                ..
            }
        ));
    }

    #[test]
    fn nested_arrays_above_max_array_nesting_are_rejected() {
        // CVE-2026-5757: cada nível de aninhamento custa 12 bytes no ficheiro, e
        // sem limite o percurso recursivo esgotaria a pilha.
        let at_limit = file(3, &[kv_body("k", 9, &nested_array_value(64))], &[]);
        let report = sanitize_gguf(&at_limit);
        assert!(
            report.ok,
            "64 arrays abertos estão no limite: {:?}",
            report.error
        );
        assert_eq!(report.max_array_nesting_seen, 64);

        // Com 65, o 65.º array começa 64 × 12 bytes depois do primeiro.
        let above = file(3, &[kv_body("k", 9, &nested_array_value(65))], &[]);
        let report = sanitize_gguf(&above);
        assert_eq!(
            rejection(&report),
            Rejection::LimitExceeded {
                limit: Limit::ArrayNesting,
                found: 65,
                max: 64,
                offset: 37 + 64 * 12,
            }
        );
        assert_eq!(
            report.kv_pairs_walked, 0,
            "a recusa acontece dentro do primeiro par, antes de ele contar"
        );
    }

    #[test]
    fn a_declared_count_that_exceeds_the_file_is_rejected() {
        // CVE-2026-5757 / CVE-2026-7482: o cabeçalho declara 99 999 pares
        // chave-valor num ficheiro de 24 bytes. 99 999 está **dentro** de
        // `max_kv_pairs`, então o que recusa é o confronto com o tamanho real —
        // e nada é percorrido nem comprometido.
        let bytes = file_declaring(3, 0, 99_999, &[]);

        let report = sanitize_gguf(&bytes);
        assert_eq!(
            rejection(&report),
            Rejection::DeclaredCountExceedsFile {
                field: Field::KvCount,
                declared: 99_999,
                minimum_bytes: 99_999 * MIN_KV_BYTES,
                available_bytes: 0,
                offset: 16,
            }
        );
        assert_eq!(report.kv_pairs_walked, 0, "nada foi percorrido");
        assert_eq!(report.decoded_metadata_bytes, 0, "nada foi comprometido");

        // O mesmo mecanismo do lado dos tensores: 1 000 000 declarados num
        // ficheiro de 24 bytes, com 1 000 000 dentro de `max_tensor_count`.
        let tensors = file_declaring(3, 1_000_000, 0, &[]);
        assert!(matches!(
            rejection(&sanitize_gguf(&tensors)),
            Rejection::DeclaredCountExceedsFile {
                field: Field::TensorCount,
                declared: 1_000_000,
                available_bytes: 0,
                ..
            }
        ));

        // E uma contagem **acima** do limite de contagem é recusada pelo limite,
        // antes ainda do confronto com o ficheiro.
        let huge = file_declaring(3, 0, 1 << 40, &[]);
        assert!(matches!(
            rejection(&sanitize_gguf(&huge)),
            Rejection::LimitExceeded {
                limit: Limit::KvPairs,
                found: 1_099_511_627_776,
                ..
            }
        ));
    }

    #[test]
    fn a_string_length_that_overflows_arithmetic_is_rejected() {
        // CVE-2026-86289 / CVE-2025-53630: um comprimento de `u64::MAX` somado
        // ao deslocamento transborda. Com o limite da string levantado, o que
        // trava não é um limite — é a aritmética verificada.
        let mut value = Vec::new();
        value.extend_from_slice(&u64::MAX.to_le_bytes());
        let bytes = file(3, &[kv_body("k", 8, &value)], &[]);
        let unlimited = SanitizeLimits {
            max_string_len: u64::MAX,
            max_metadata_size: u64::MAX,
            max_decoded_metadata_size: u64::MAX,
            ..SanitizeLimits::default()
        };
        let report = sanitize_gguf_with_limits(&bytes, &unlimited);
        assert_eq!(
            rejection(&report),
            Rejection::ArithmeticOverflow {
                field: Field::StringBytes,
                offset: 45,
            }
        );

        // E a multiplicação de um array: 2^62 elementos de 4 bytes é 2^64, que
        // não cabe num `u64`. Com o limite de elementos levantado, quem recusa é
        // a multiplicação verificada.
        let bytes = file(3, &[kv_array_declaring("k", 4, 1 << 62)], &[]);
        let wide = SanitizeLimits {
            max_array_elements: u64::MAX,
            ..unlimited
        };
        let report = sanitize_gguf_with_limits(&bytes, &wide);
        assert_eq!(
            rejection(&report),
            Rejection::ArithmeticOverflow {
                field: Field::ArrayElement,
                offset: 37,
            },
            "a multiplicação count × largura tem de ser verificada"
        );
    }

    #[test]
    fn a_tensor_with_more_than_four_dimensions_is_rejected() {
        // CVE-2026-53923: o número de dimensões é limitado antes de ser usado
        // como contagem do laço das dimensões. O campo está no deslocamento 33
        // (24 + 8 do nome + 1 do nome).
        let above = file(3, &[], &[tensor("t", &[1, 2, 3, 4, 5], 0, 0)]);
        let report = sanitize_gguf(&above);
        assert_eq!(
            rejection(&report),
            Rejection::LimitExceeded {
                limit: Limit::TensorDims,
                found: 5,
                max: 4,
                offset: 33,
            }
        );
        assert_eq!(report.tensors_walked, 0);

        // Com quatro dimensões o mesmo ficheiro passa, com o padding e o dado
        // que um tensor exige para o deslocamento validar.
        let at_limit = file_with_one_tensor("t", &[1, 2, 3, 4], 0, 96);
        assert!(sanitize_gguf(&at_limit).ok);
    }

    #[test]
    fn a_tensor_dimension_above_u32_is_rejected() {
        // CVE-2026-53923, a outra metade: uma dimensão `u64` maior que `u32` é
        // exatamente o valor que um kernel que a receba em `u32` trunca. A
        // primeira dimensão está no deslocamento 37 (24 + 8 + 1 + 4).
        let bytes = file_with_one_tensor("t", &[u32::MAX as u64 + 1], 0, 96);

        let report = sanitize_gguf(&bytes);
        assert_eq!(
            rejection(&report),
            Rejection::LimitExceeded {
                limit: Limit::TensorDimValue,
                found: u32::MAX as u64 + 1,
                max: u32::MAX as u64,
                offset: 37,
            }
        );

        let at_limit = file_with_one_tensor("t", &[u32::MAX as u64], 0, 96);
        assert!(sanitize_gguf(&at_limit).ok);
    }

    #[test]
    fn a_tensor_offset_beyond_the_data_is_rejected() {
        // CVE-2026-7482: um deslocamento declarado que aponta para fora da
        // secção de dados é recusado. O ficheiro tem 96 bytes de dado e o tensor
        // declara 100 000.
        let bytes = file_with_one_tensor("t", &[4], 100_000, 96);

        let report = sanitize_gguf(&bytes);
        match rejection(&report) {
            Rejection::Truncated {
                field: Field::TensorOffset,
                end,
                available,
                ..
            } => {
                assert_eq!(available, bytes.len() as u64);
                assert_eq!(end, report.data_offset.expect("data_offset") + 100_000);
            }
            other => panic!("a causa tem de ser o deslocamento do tensor: {other:?}"),
        }

        // Com o deslocamento dentro do dado disponível, passa.
        let ok = file_with_one_tensor("t", &[4], 96, 96);
        assert!(sanitize_gguf(&ok).ok, "{:?}", sanitize_gguf(&ok).error);
    }

    #[test]
    fn a_compact_array_that_expands_past_the_decoded_budget_is_rejected() {
        // CVE-2026-7482 / CVE-2026-65315, e a razão de existirem dois
        // orçamentos: 200 bytes `U8` são 200 bytes no ficheiro e 1 600 bytes
        // descodificados. Com o orçamento descodificado apertado e o da secção
        // folgado, quem recusa é o descodificado.
        let bytes = file(3, &[kv_array_of_u8("k", 200)], &[]);
        let limits = SanitizeLimits {
            max_metadata_size: 1 << 20,
            max_decoded_metadata_size: 1_024,
            ..SanitizeLimits::default()
        };

        let report = sanitize_gguf_with_limits(&bytes, &limits);
        match rejection(&report) {
            Rejection::LimitExceeded {
                limit: Limit::DecodedMetadataSize,
                found,
                max: 1_024,
                ..
            } => {
                assert_eq!(
                    found, 1_621,
                    "9 da chave + 12 do cabeçalho do array + 1 600 dos elementos"
                );
                assert!(
                    report.metadata_bytes < 1 << 20,
                    "a secção lida do ficheiro está dentro do orçamento dela: {}",
                    report.metadata_bytes
                );
            }
            other => panic!("a causa tem de ser o orçamento descodificado: {other:?}"),
        }

        let generous = SanitizeLimits {
            max_decoded_metadata_size: 2_048,
            ..limits
        };
        assert!(sanitize_gguf_with_limits(&bytes, &generous).ok);
    }

    #[test]
    fn a_metadata_section_above_max_metadata_size_is_rejected() {
        // O outro lado da independência: o orçamento da secção **lida** é que
        // morde, com o descodificado folgado.
        let bytes = file(3, &[kv_array_of_u8("k", 200)], &[]);
        let limits = SanitizeLimits {
            max_metadata_size: 64,
            max_decoded_metadata_size: 1 << 20,
            ..SanitizeLimits::default()
        };

        let report = sanitize_gguf_with_limits(&bytes, &limits);
        match rejection(&report) {
            Rejection::LimitExceeded {
                limit: Limit::MetadataSize,
                found,
                max: 64,
                ..
            } => {
                assert_eq!(found, (bytes.len() - HEADER_LEN) as u64);
                assert!(
                    report.decoded_metadata_bytes < 1 << 20,
                    "o orçamento descodificado está folgado: {}",
                    report.decoded_metadata_bytes
                );
            }
            other => panic!("a causa tem de ser o orçamento da secção lida: {other:?}"),
        }

        let generous = SanitizeLimits {
            max_metadata_size: 4_096,
            ..limits
        };
        assert!(sanitize_gguf_with_limits(&bytes, &generous).ok);
    }

    #[test]
    fn the_decoded_budget_is_reachable_with_the_default_limits() {
        // A versão do teste anterior com os limites **por omissão**, para que a
        // afirmação do módulo ("é a soma dos arrays que chega ao orçamento
        // descodificado") seja medida e não deduzida. Cada array traz um milhão
        // de bytes `U8`: 1 MB lido, 8 MB descodificados, por array.
        //
        //   34 × 8 000 000 = 272 000 000 > 268 435 456 (o limite descodificado)
        //   34 × 1 000 0xx ≈ 34 MB < 268 435 456 (o limite dos bytes lidos)
        //
        // O array é o mesmo ficheiro 34 vezes, e o que muda entre os dois casos
        // é só a contagem de pares.
        let one_array = kv_array_of_u8("k", 1_000_000);

        let at_limit: Vec<Vec<u8>> = (0..33).map(|_| one_array.clone()).collect();
        let report = sanitize_gguf(&file(3, &at_limit, &[]));
        assert!(
            report.ok,
            "33 arrays descodificam 264 000 000 bytes, abaixo do limite: {:?}",
            report.error
        );
        assert_eq!(report.decoded_metadata_bytes, 33 * 8_000_000 + 33 * 9 + 33 * 12);

        let above: Vec<Vec<u8>> = (0..34).map(|_| one_array.clone()).collect();
        let report = sanitize_gguf(&file(3, &above, &[]));
        match rejection(&report) {
            Rejection::LimitExceeded {
                limit: Limit::DecodedMetadataSize,
                found,
                max,
                ..
            } => {
                assert_eq!(found, 34 * 8_000_000 + 34 * 9 + 34 * 12);
                assert_eq!(max, 256 * 1024 * 1024);
                assert!(
                    report.metadata_bytes < 36 * 1024 * 1024,
                    "os bytes lidos ficaram muito abaixo do orçamento deles: {}",
                    report.metadata_bytes
                );
            }
            other => panic!("a causa tem de ser o orçamento descodificado: {other:?}"),
        }
    }

    #[test]
    fn more_kv_pairs_than_max_kv_pairs_is_rejected() {
        // CVE-2026-65315. O limite é conferido sobre a contagem declarada, antes
        // de o ficheiro ser percorrido ou de qualquer byte ser comprometido.
        let many: Vec<Vec<u8>> = (0..4)
            .map(|index| kv_u32(&format!("k{index}"), 1))
            .collect();
        let bytes = file(3, &many, &[]);

        let limits = SanitizeLimits {
            max_kv_pairs: 3,
            ..SanitizeLimits::default()
        };
        let report = sanitize_gguf_with_limits(&bytes, &limits);
        assert_eq!(
            rejection(&report),
            Rejection::LimitExceeded {
                limit: Limit::KvPairs,
                found: 4,
                max: 3,
                offset: 24,
            }
        );
        assert_eq!(report.kv_pairs_walked, 0);

        let generous = SanitizeLimits {
            max_kv_pairs: 4,
            ..limits
        };
        assert!(sanitize_gguf_with_limits(&bytes, &generous).ok);
    }

    #[test]
    fn more_tensors_than_max_tensor_count_is_rejected() {
        // CVE-2026-65315, pelo mesmo mecanismo, do lado dos tensores.
        let bytes = file(3, &[], &[tensor("a", &[1], 0, 0), tensor("b", &[1], 0, 0)]);

        let limits = SanitizeLimits {
            max_tensor_count: 1,
            ..SanitizeLimits::default()
        };
        let report = sanitize_gguf_with_limits(&bytes, &limits);
        assert_eq!(
            rejection(&report),
            Rejection::LimitExceeded {
                limit: Limit::TensorCount,
                found: 2,
                max: 1,
                offset: 24,
            }
        );
        assert_eq!(report.tensors_walked, 0);
    }

    #[test]
    fn a_truncated_metadata_section_is_rejected() {
        let full = file(3, &[kv_string("k", "valor"), kv_u32("n", 7)], &[]);
        assert_eq!(full.len(), 67, "o layout do teste: 24 + 25 + 18");

        // O corte do último byte deixa o valor do segundo par pela metade.
        let report = sanitize_gguf(&full[..full.len() - 1]);
        assert_eq!(
            rejection(&report),
            Rejection::Truncated {
                field: Field::Value,
                offset: 63,
                end: 67,
                available: 66,
            }
        );
        assert_eq!(report.kv_pairs_walked, 1, "o primeiro par foi percorrido");
        assert!(report.metadata_bytes > 0);
    }

    #[test]
    fn an_unknown_metadata_value_type_is_rejected() {
        // O tipo 13 não existe em GGUF. Recusá-lo é o que impede que o percurso
        // trate um campo desconhecido como se soubesse a largura dele.
        for value_type in [13u32, 99, u32::MAX] {
            let bytes = file(3, &[kv_body("k", value_type, &[0; 8])], &[]);
            let report = sanitize_gguf(&bytes);
            assert!(
                matches!(
                    rejection(&report),
                    Rejection::UnknownMetadataValueType { value_type: found, offset: 37 }
                        if found == value_type
                ),
                "tipo {value_type}"
            );
        }

        // E o mesmo dentro de um array: o tipo do elemento é conferido.
        let bytes = file(3, &[kv_array_declaring("k", 13, 1)], &[]);
        assert!(matches!(
            rejection(&sanitize_gguf(&bytes)),
            Rejection::UnknownMetadataValueType {
                value_type: 13,
                offset: 37,
            }
        ));
    }

    #[test]
    fn a_zero_alignment_is_rejected() {
        // Um alinhamento zero é um divisor zero à espera de acontecer: o padding
        // é calculado com `% alignment`. O valor de `general.alignment` (17
        // bytes de chave) está no deslocamento 53 = 24 + 8 + 17 + 4.
        let bytes = file(
            3,
            &[kv_u32("general.alignment", 0)],
            &[tensor("t", &[1], 0, 0)],
        );

        let report = sanitize_gguf(&bytes);
        assert_eq!(rejection(&report), Rejection::ZeroAlignment { offset: 53 });

        // Com um alinhamento válido o mesmo ficheiro passa, com o padding que o
        // alinhamento exige.
        let mut ok = file(
            3,
            &[kv_u32("general.alignment", 32)],
            &[tensor("t", &[1], 0, 0)],
        );
        let padding = (32 - ok.len() % 32) % 32;
        ok.resize(ok.len() + padding, 0);
        let report = sanitize_gguf(&ok);
        assert!(report.ok, "erro: {:?}", report.error);
        assert_eq!(report.alignment, Some(32));
        assert_eq!(report.data_offset, Some(ok.len() as u64));
    }

    #[test]
    fn an_alignment_that_is_not_an_integer_is_rejected() {
        // Sem saber o alinhamento não se localiza a secção de dados, e supor 32
        // seria medir contra uma estrutura que o ficheiro não declara.
        let bytes = file(3, &[kv_string("general.alignment", "32")], &[]);

        let report = sanitize_gguf(&bytes);
        assert_eq!(
            rejection(&report),
            Rejection::AlignmentNotAnInteger {
                value_type: 8,
                offset: 53,
            }
        );
    }

    #[test]
    fn a_file_that_is_not_gguf_is_rejected() {
        // O Gate 0 é o primeiro gate: um ficheiro que não é GGUF é recusado por
        // ele, com os bytes encontrados no relatório.
        let report = sanitize_gguf(b"nao e um gguf, de certeza");
        assert_eq!(
            rejection(&report),
            Rejection::BadMagic {
                found: "6e616f20".to_string(),
            }
        );
        assert_eq!(report.version, None);

        // Menos de quatro bytes não é um magic errado: é um ficheiro truncado.
        for short in [&b""[..], b"G", b"GG", b"GGU"] {
            let report = sanitize_gguf(short);
            assert!(matches!(
                rejection(&report),
                Rejection::Truncated {
                    field: Field::Magic,
                    offset: 0,
                    end: 4,
                    ..
                }
            ));
        }

        // E a forma de 16 bytes que o `gguf.rs` documenta — magic e versão
        // presentes, o contador de pares truncado — é recusada como truncada.
        let short_header = header(3, 0, 0);
        let report = sanitize_gguf(&short_header[..16]);
        assert_eq!(
            rejection(&report),
            Rejection::Truncated {
                field: Field::KvCount,
                offset: 0,
                end: 24,
                available: 16,
            }
        );
    }

    #[test]
    fn a_version_that_is_not_interpreted_is_rejected() {
        for version in [0u32, 1, 4, 99, 0x0300_0000] {
            let report = sanitize_gguf(&header(version, 0, 0));
            assert!(
                matches!(
                    rejection(&report),
                    Rejection::UnsupportedVersion { version: found } if found == version
                ),
                "versão {version}"
            );
        }
        assert!(sanitize_gguf(&header(2, 0, 0)).ok, "a versão 2 é interpretada");
        assert!(sanitize_gguf(&header(3, 0, 0)).ok, "a versão 3 é interpretada");
    }

    #[test]
    fn a_negative_count_is_rejected_rather_than_widened() {
        // O campo é um `i64` no fio: -1 não é um `u64` de 18 quintiliões.
        let negative_tensors = header(3, u64::MAX, 0);
        assert_eq!(
            rejection(&sanitize_gguf(&negative_tensors)),
            Rejection::NegativeCount {
                field: Field::TensorCount,
            }
        );

        let negative_kvs = header(3, 0, u64::MAX);
        assert_eq!(
            rejection(&sanitize_gguf(&negative_kvs)),
            Rejection::NegativeCount {
                field: Field::KvCount,
            }
        );
    }

    // --- propriedades do percurso -----------------------------------------

    /// As frases que o CLI imprime quando recusa. Cada caso é a entrada da
    /// variante e o fragmento que a frase tem de conter: um veredito negativo
    /// cuja frase não diga o que falhou é um veredito que o utilizador não
    /// consegue usar.
    #[test]
    fn every_rejection_says_what_failed_in_its_message() {
        let cases: Vec<(Rejection, &str)> = vec![
            (
                rejection(&sanitize_gguf(b"XXXX........")),
                "magic inválido: esperado `GGUF`, encontrado 58585858",
            ),
            (
                rejection(&sanitize_gguf(&header(99, 0, 0))),
                "versão 99 não interpretada",
            ),
            (
                rejection(&sanitize_gguf(&header(3, 0, 0)[..16])),
                "truncado: `contador de pares chave-valor`",
            ),
            (
                rejection(&sanitize_gguf(&header(3, u64::MAX, 0))),
                "`contador de tensores` é negativo",
            ),
            (
                rejection(&sanitize_gguf(&file(3, &[kv_string("k", &"x".repeat(65_536))], &[]))),
                "limite `max_string_len` excedido: 65536 > 65535",
            ),
            (
                rejection(&sanitize_gguf(&file(
                    3,
                    &[kv_array_declaring("k", 0, 1_000_001)],
                    &[],
                ))),
                "limite `max_array_elements` excedido: 1000001 > 1000000",
            ),
            (
                rejection(&sanitize_gguf(&file(
                    3,
                    &[kv_body("k", 9, &nested_array_value(65))],
                    &[],
                ))),
                "limite `max_array_nesting` excedido: 65 > 64",
            ),
            (
                rejection(&sanitize_gguf(&file(3, &[kv_body("k", 13, &[0; 4])], &[]))),
                "tipo de valor de metadados desconhecido: 13",
            ),
            (
                rejection(&sanitize_gguf(&file_declaring(3, 0, 99_999, &[]))),
                "declara 99999, que exige no mínimo 1299987 byte(s), e o ficheiro só tem 0 byte(s)",
            ),
            (
                rejection(&sanitize_gguf(&file(
                    3,
                    &[kv_u32("general.alignment", 0)],
                    &[],
                ))),
                "`general.alignment` declarado como zero",
            ),
            (
                rejection(&sanitize_gguf(&file(
                    3,
                    &[kv_string("general.alignment", "32")],
                    &[],
                ))),
                "`general.alignment` declarado com o tipo 8, que não é inteiro",
            ),
        ];

        for (cause, expected) in cases {
            let message = cause.to_string();
            assert!(
                message.contains(expected),
                "a frase de {cause:?} devia conter `{expected}`, e é `{message}`"
            );
        }
    }

    #[test]
    fn every_truncation_of_a_valid_file_is_rejected_via_a_report_not_a_panic() {
        // A varredura toda: nenhum prefixo do ficheiro pode ser aceito nem
        // entrar em pânico. O corte antes de `data_offset` tem de ser recusado
        // *sempre*; a partir dali, o que falta é dado de tensor, e o
        // deslocamento declarado (0) continua dentro do que resta — o Gate 0 não
        // mede a extensão dos dados, e diz isso no topo do módulo.
        let full = file_with_one_tensor("t", &[2, 3], 0, 96);
        let data_offset = sanitize_gguf(&full)
            .data_offset
            .expect("o ficheiro completo tem secção de dados") as usize;

        for len in 0..full.len() {
            let report = sanitize_gguf(&full[..len]);
            assert_eq!(
                report.ok,
                report.rejection.is_none(),
                "o veredito e a causa têm de concordar no prefixo de {len} bytes"
            );
            if len < data_offset {
                assert!(
                    !report.ok,
                    "o prefixo de {len} bytes corta os metadados e tem de ser recusado"
                );
            }
        }
    }

    #[test]
    fn adversarial_inputs_never_panic_and_never_widen() {
        let absurd_string = [
            &1u64.to_le_bytes()[..],
            b"k",
            &8u32.to_le_bytes(),
            &u64::MAX.to_le_bytes(),
        ]
        .concat();
        let mut cases: Vec<Vec<u8>> = vec![
            Vec::new(),
            b"G".to_vec(),
            b"GGUF".to_vec(),
            vec![0xFF; 24],
            vec![0x00; 24],
            header(3, u64::MAX, u64::MAX),
            header(3, 1, 0),
            header(3, 0, 1),
            header(2, 0, 0),
            header(3, 0, 0),
            // Um par cujo comprimento de chave é `u64::MAX`.
            file_declaring(3, 0, 1, &u64::MAX.to_le_bytes()),
            // Um par cujo comprimento de valor string é `u64::MAX`.
            file_declaring(3, 0, 1, &absurd_string),
            // Um par que declara um array de 2^63 elementos.
            file_declaring(3, 0, 1, &kv_array_declaring("k", 0, 1 << 63)),
        ];

        let valid = file_with_one_tensor("t", &[2, 3], 0, 32);
        for len in 0..valid.len() {
            cases.push(valid[..len].to_vec());
        }

        for case in cases {
            let report = sanitize_gguf(&case);
            assert_eq!(
                report.ok,
                report.rejection.is_none(),
                "veredito e causa discordam em {} byte(s)",
                case.len()
            );
            if !report.ok {
                assert!(report.error.is_some(), "uma recusa tem de ter frase");
            }
        }
    }

    #[test]
    fn the_api_is_over_bytes_and_a_short_buffer_is_not_an_error() {
        // Nenhuma função deste módulo recebe um caminho nem devolve `Err`: um
        // ficheiro malformado é um relatório. Este teste existe para fixar essa
        // forma — se alguém trocar a assinatura por um `Path` ou por um
        // `Result`, ele para de compilar.
        let unknown_type = file(3, &[kv_body("k", 200, &[0; 4])], &[]);
        let empty_header = header(3, 0, 0);
        let inputs: Vec<&[u8]> = vec![&[], b"G", &empty_header, FIXTURE, &unknown_type];

        for bytes in inputs {
            let report: SanitizeReport = sanitize_gguf(bytes);
            assert_eq!(report.ok, report.rejection.is_none());
        }
    }
}
