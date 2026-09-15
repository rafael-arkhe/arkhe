//! `arkhe-mcp` — Fase 3 do Plano Arkhe OS: um servidor **MCP** (Model Context
//! Protocol) por stdio que expõe a verificação do Arkhe a agentes de IA.
//!
//! # O que este crate é, e o que ele não é
//!
//! Ele **não** verifica nada. As quatro ferramentas que o servidor publica são
//! cascas finas sobre funções que já existem em [`arkhe_verify`] — a casca
//! nativa do core de verificação. A tabela abaixo é o mapa completo; cada linha
//! nomeia a função realmente chamada, e nenhuma delas é reimplementada aqui:
//!
//! | Ferramenta MCP | Delega a | Relatório |
//! |:---|:---|:---|
//! | `arkhe_verify_sha256` | [`arkhe_verify::verify_sha256`] | `Sha256Report` |
//! | `arkhe_verify_signature` | [`arkhe_verify::verify_signature`] | `SignatureReport` |
//! | `arkhe_verify_inclusion` | [`arkhe_verify::verify_inclusion`] | `InclusionReport` |
//! | `arkhe_verify_attestation` | [`arkhe_verify::verify_attestation`] | `AttestationReport` |
//!
//! A quinta verificação do core — o quórum de witnesses
//! ([`arkhe_verify::verify_witness_quorum`]) — **não** é publicada como
//! ferramenta própria: ela já é avaliada, e reportada no campo `quorum`, pelo
//! pipeline de [`arkhe_verify::verify_attestation`], que é a ferramenta
//! `arkhe_verify_attestation`.
//!
//! ## Por que o quórum não tem ferramenta própria
//!
//! A decisão é de escopo — as quatro ferramentas publicadas são as que cobrem
//! as verificações sem repetir trabalho —, e o custo dela é real e vale
//! registrar em vez de esconder: pelo pipeline, o quórum só se vê como
//! booleano. Duas informações que a função do core **devolve** ficam fora de
//! alcance:
//!
//! - `valid_witnesses` — quantos witnesses distintos, confiáveis e com
//!   assinatura válida de fato assinaram, que a casca nativa reporta mesmo
//!   quando o veredito é negativo (é a margem do quórum).
//! - a causa específica de um `threshold` abaixo do mínimo aceito pelo core
//!   (`QUORUM_MIN`), que o pipeline reduz a `quorum: false`.
//!
//! Nada disso é uma lacuna de verificação: quem chama o pipeline recebe o
//! veredito correto do quórum. Se a contagem for necessária, a ferramenta que
//! falta é uma delegação de uma linha a
//! [`arkhe_verify::verify_witness_quorum`] — o mesmo tipo de casca que as
//! outras quatro são. Ela não foi escrita porque quatro é o teto do escopo
//! desta fase, não porque o core não a ofereça.
//!
//! # As três camadas
//!
//! ```text
//! src/main.rs    o binário: fala stdio, serve o handler, e nada mais
//! src/server.rs  o handler MCP: registra as tools, gera os schemas, responde
//! src/tools.rs   as ferramentas: argumentos, e a chamada ao `arkhe-verify`
//! ```
//!
//! A separação é deliberada e tem uma consequência prática: [`tools`] não
//! conhece o `rmcp`. As funções de lá recebem argumentos já desserializados e
//! devolvem um [`tools::ToolOutcome`], então os testes as exercitam
//! **diretamente**, sem precisar de um cliente MCP vivo — o mesmo motivo pelo
//! qual o core do `arkhe-verify-wasm` é testável no host.
//!
//! # Duas espécies de "não passou"
//!
//! A distinção que este crate preserva com cuidado:
//!
//! - **Um veredito negativo é um resultado.** A prova de inclusão não fecha, a
//!   assinatura não confere, o quórum não foi atingido: a verificação *rodou* e
//!   o relatório diz por quê. Isso é devolvido como um resultado normal, com o
//!   relatório do `arkhe-verify` inteiro — inclusive `valid_witnesses` e
//!   `trusted`, que um booleano perderia.
//! - **Uma entrada que não se interpreta é uma rejeição.** O payload não é
//!   base64, a prova não é hex, o trust root não é um array de chaves de 32
//!   bytes: a verificação **não chegou a rodar**. Isso é devolvido como erro
//!   estruturado (`stage: "input"`), porque não existe relatório de verificação
//!   para algo que nunca foi verificado — inventar um seria fingir que o core
//!   respondeu.
//!
//! Note onde cai a fronteira: o `trust_root` é interpretado por
//! [`arkhe_verify::TrustRoot`], o tipo do core — a fachada nativa recebe um
//! trust root já tipado justamente para que um trust root malformado **não
//! tenha como chegar** a uma verificação. A rejeição na fronteira é o
//! comportamento documentado do core, não uma decisão desta crate.
//!
//! Um caso que parece da primeira espécie e é da segunda, mas não é desta
//! crate: um documento de atestação que não é JSON é reportado **pelo core**
//! como relatório reprovado (os quatro estágios em `false` mais a causa), porque
//! `verify_attestation` recebe a atestação como `&str` JSON e esse formato *é* a
//! interface do pipeline. Aqui esse caso é um resultado, não uma rejeição.

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod server;
pub mod tools;

pub use server::ArkheMcp;
