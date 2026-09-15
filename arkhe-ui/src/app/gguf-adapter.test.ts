/**
 * A fiação do IPC — o teste que prova que o `src/app/` chama mesmo
 * `inspect_gguf_model`, e que se porta bem quando **não** há Tauri.
 *
 * # O que um mock prova, e o que não prova
 *
 * Isto prova a **fiação**: que o nome do comando, a forma dos argumentos e a
 * leitura da resposta são os que o `src-tauri/src/lib.rs` declara; que o
 * `invoke` é chamado quando há runtime e **não** é chamado quando não há; e que
 * a degradação fora do Tauri é um caminho próprio, e não um `catch` genérico.
 *
 * Isto **não** prova que o IPC real funciona. Um mock substitui precisamente a
 * peça que só existe dentro de um webview — `window.__TAURI_INTERNALS__` — e
 * portanto não diz nada sobre o runtime. Essa parte exige `npm run tauri:dev`
 * numa sessão gráfica; nenhum teste deste ficheiro a substitui.
 *
 * Os valores esperados vêm das constantes conhecidas do NIST (`SHA-256("")`),
 * como no lado Rust, para que o teste não seja a implementação a concordar
 * consigo mesma.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';

// `vi.hoisted` e não `const` no escopo do ficheiro: as fábricas do `vi.mock` são
// içadas para cima dos `import`, e os `import` são avaliados antes de qualquer
// `const` deste ficheiro — referenciar um `const` de fora daria
// "Cannot access before initialization".
const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  isTauri: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: mocks.invoke,
  isTauri: mocks.isTauri,
}));

import { inspectGgufModel, nativeInspectionAvailable, parseInspection } from './gguf-adapter';

const CAMINHO = 'C:/modelos/arkhe.gguf';

/** `SHA-256("")`, do NIST — a mesma constante que o teste de `src-tauri`. */
const SHA256_VAZIO = 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855';

/** Uma resposta com a forma que `ModelInspection` (Rust) serializa. */
const RESPOSTA_VALIDA = {
  path: CAMINHO,
  bytes: 217204,
  digest_hex: SHA256_VAZIO,
  header: {
    ok: true,
    magic_ok: true,
    available_bytes: 217204,
    version: 3,
    tensor_count: 0,
    metadata_kv_count: 0,
    error: null,
  },
};

beforeEach(() => {
  mocks.invoke.mockReset();
  mocks.isTauri.mockReset();
});

describe('inspectGgufModel — dentro do Tauri', () => {
  beforeEach(() => {
    mocks.isTauri.mockReturnValue(true);
  });

  it('chama `inspect_gguf_model` com o caminho e devolve a inspecção', async () => {
    mocks.invoke.mockResolvedValue(RESPOSTA_VALIDA);

    const resultado = await inspectGgufModel(CAMINHO);

    // O nome do comando e a chave do argumento têm de ser exactamente os que o
    // `#[tauri::command] fn inspect_gguf_model(path: String)` espera: um `path`
    // mal escrito não é um erro de tipo, é uma rejeição em tempo de execução.
    expect(mocks.invoke).toHaveBeenCalledTimes(1);
    expect(mocks.invoke).toHaveBeenCalledWith('inspect_gguf_model', { path: CAMINHO });

    expect(resultado).toEqual({
      kind: 'inspected',
      inspection: {
        path: CAMINHO,
        bytes: 217204,
        digest_hex: SHA256_VAZIO,
        header: {
          ok: true,
          magic_ok: true,
          available_bytes: 217204,
          version: 3,
          tensor_count: 0,
          metadata_kv_count: 0,
          error: null,
        },
      },
    });
  });

  it('o eco do `path` é o que foi pedido — a correlação de respostas é possível', async () => {
    mocks.invoke.mockResolvedValue({ ...RESPOSTA_VALIDA, path: CAMINHO });

    const resultado = await inspectGgufModel(CAMINHO);
    expect(resultado.kind === 'inspected' && resultado.inspection.path).toBe(CAMINHO);
  });

  it('conteúdo inválido é um RESULTADO, não uma falha — digest e cabeçalho separados', async () => {
    // O caso do ficheiro truncado: digest calculável, cabeçalho recusado. O
    // comando só devolve `Err` quando a leitura falha; isso tem de sobreviver à
    // fronteira do IPC em vez de colapsar num erro genérico.
    mocks.invoke.mockResolvedValue({
      path: CAMINHO,
      bytes: 16,
      digest_hex: SHA256_VAZIO,
      header: {
        ok: false,
        magic_ok: false,
        available_bytes: 16,
        version: null,
        tensor_count: null,
        metadata_kv_count: null,
        error: 'magic inválido: esperado "GGUF"',
      },
    });

    const resultado = await inspectGgufModel(CAMINHO);

    expect(resultado.kind).toBe('inspected');
    if (resultado.kind !== 'inspected') throw new Error('esperava `inspected`');

    expect(resultado.inspection.header.ok).toBe(false);
    // O digest existe e é mostrado apesar de o cabeçalho ter falhado.
    expect(resultado.inspection.digest_hex).toBe(SHA256_VAZIO);
    expect(resultado.inspection.header.error).toContain('magic');
    // `null` é "ausente", não zero.
    expect(resultado.inspection.header.version).toBeNull();
    expect(resultado.inspection.header.tensor_count).toBeNull();
  });

  it('o `Err` do comando chega como STRING e vira `failed`, não uma excepção', async () => {
    // É o que o Rust produz numa falha de leitura:
    // `Err(format!("não foi possível ler {path}: {err}"))`.
    mocks.invoke.mockRejectedValue(`não foi possível ler ${CAMINHO}: o sistema não consegue encontrar o ficheiro`);

    const resultado = await inspectGgufModel(CAMINHO);

    expect(resultado.kind).toBe('failed');
    if (resultado.kind !== 'failed') throw new Error('esperava `failed`');
    expect(resultado.message).toContain('não foi possível ler');
  });

  it('uma resposta com forma inesperada é `failed` — nada é adivinhado', async () => {
    mocks.invoke.mockResolvedValue({ path: CAMINHO, header: {} });

    const resultado = await inspectGgufModel(CAMINHO);

    expect(resultado.kind).toBe('failed');
    // O ponto: não devolve `bytes: 0` nem um digest vazio como se fossem medidos.
    if (resultado.kind !== 'failed') throw new Error('esperava `failed`');
    expect(resultado.message).toContain('forma declarada');
  });
});

describe('inspectGgufModel — fora do Tauri', () => {
  beforeEach(() => {
    mocks.isTauri.mockReturnValue(false);
  });

  it('devolve `unavailable` e NÃO chama o `invoke`', async () => {
    const resultado = await inspectGgufModel(CAMINHO);

    expect(resultado).toEqual({ kind: 'unavailable' });
    // A asserção que importa: o `invoke` desreferencia
    // `window.__TAURI_INTERNALS__` e lançaria no navegador. A guarda é o que
    // impede que a app quebre fora do Tauri — e é isso que aqui se mede.
    expect(mocks.invoke).not.toHaveBeenCalled();
  });

  it('não lança: o caminho de degradação é um valor, não uma excepção', async () => {
    await expect(inspectGgufModel(CAMINHO)).resolves.toEqual({ kind: 'unavailable' });
  });

  it('`unavailable` não é `failed` — não há erro nenhum a reportar', async () => {
    const resultado = await inspectGgufModel(CAMINHO);
    expect(resultado.kind).not.toBe('failed');
  });

  it('`nativeInspectionAvailable()` reflecte o runtime, sem cache', () => {
    expect(nativeInspectionAvailable()).toBe(false);

    mocks.isTauri.mockReturnValue(true);
    // Lido em tempo de chamada: se fosse capturado numa constante de módulo,
    // isto continuaria `false`.
    expect(nativeInspectionAvailable()).toBe(true);
  });
});

describe('parseInspection', () => {
  it('interpreta a forma declarada', () => {
    expect(parseInspection(RESPOSTA_VALIDA)).toEqual(RESPOSTA_VALIDA);
  });

  it('recusa o que não é objecto', () => {
    for (const entrada of [null, undefined, 42, 'texto', [], true]) {
      expect(parseInspection(entrada)).toBeNull();
    }
  });

  it('recusa campos obrigatórios em falta ou com o tipo errado', () => {
    expect(parseInspection({ ...RESPOSTA_VALIDA, bytes: '217204' })).toBeNull();
    expect(parseInspection({ ...RESPOSTA_VALIDA, digest_hex: undefined })).toBeNull();
    expect(parseInspection({ ...RESPOSTA_VALIDA, path: null })).toBeNull();
    expect(parseInspection({ ...RESPOSTA_VALIDA, header: null })).toBeNull();
    expect(
      parseInspection({ ...RESPOSTA_VALIDA, header: { ...RESPOSTA_VALIDA.header, ok: 'true' } }),
    ).toBeNull();
  });

  it('recusa uma `error` presente mas ilegível, em vez de engolir a causa', () => {
    expect(
      parseInspection({ ...RESPOSTA_VALIDA, header: { ...RESPOSTA_VALIDA.header, error: 7 } }),
    ).toBeNull();
  });

  it('`null` e ausente são a mesma coisa nos campos opcionais — e não zero', () => {
    const interpretado = parseInspection({
      ...RESPOSTA_VALIDA,
      header: { ...RESPOSTA_VALIDA.header, version: null, tensor_count: null },
    });

    expect(interpretado).not.toBeNull();
    expect(interpretado?.header.version).toBeNull();
    expect(interpretado?.header.tensor_count).toBeNull();
  });
});
