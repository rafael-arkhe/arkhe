/**
 * A fiação do IPC do **plugin de actualização** — o teste que prova que o
 * `src/app/` chama mesmo `plugin:hashtree-updater|check`, que os cinco estados
 * são cinco, e que fora do Tauri o `invoke` **não** é chamado.
 *
 * # O que um mock prova, e o que não prova
 *
 * Prova a **fiação**: que o nome do comando é o que o plugin registou
 * (`invoke_handler` em `src/lib.rs` do crate, `commands::check`); que a
 * resposta é lida campo a campo em vez de assumida; e que a degradação fora do
 * Tauri é um ramo próprio — não um `catch` genérico que engole tudo.
 *
 * **Não** prova que a consulta real funciona. O mock substitui precisamente a
 * peça que só existe dentro do webview (`window.__TAURI_INTERNALS__`) e, do
 * outro lado, os relays que o plugin precisa de alcançar. Nenhum teste deste
 * ficheiro substitui uma execução a sério — e hoje essa execução **não é
 * possível**: a query de resolução da árvore não é respondida por nenhum dos
 * três relays (ver o relatório). O que estes testes fixam é o que a app mostra
 * quando isso acontecer.
 *
 * O texto de erro do caso de rede não é inventado: é o que o `htree cat`
 * devolveu, literal, com os três relays em timeout.
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

// `Channel` entra no mock porque o glue copiado do crate o importa ao nível do
// módulo (é o canal do `downloadAndInstall`). Sem ele o import do módulo sob
// teste resolveria para `undefined` — inofensivo, mas uma fábrica de mock que
// não corresponde à forma do módulo real é uma mentira pequena que se paga
// depois.
vi.mock('@tauri-apps/api/core', () => ({
  invoke: mocks.invoke,
  isTauri: mocks.isTauri,
  Channel: class {},
}));

import { checkForUpdate, parseUpdateInfo, updaterRuntimeAvailable } from './updater-adapter';

/** O nome exacto do comando, como o plugin o regista. */
const COMANDO = 'plugin:hashtree-updater|check';

/** Uma resposta com a forma que `UpdateMetadata` (Rust) serializa. */
const ACTUALIZACAO = {
  currentVersion: '0.2.0',
  version: '0.2.1',
  assetName: 'arkhe-ui-app.exe',
  assetKind: 'binary',
  notes: 'liga o `check()` do plugin à interface',
  publishedAt: '2026-09-15T12:00:00Z',
  updateAvailable: true,
};

/** A mesma forma, mas com a versão publicada já igual à corrente. */
const SEM_NOVIDADE = {
  ...ACTUALIZACAO,
  version: '0.2.0',
  updateAvailable: false,
};

/** O que o `htree cat` devolveu, literal, com os relays em baixo. */
const ERRO_DE_RELAY =
  'hashtree resolver: Failed to get events from configured relays: ' +
  'wss://relay.primal.net: recv message response timeout; ' +
  'wss://relay.damus.io: recv message response timeout; ' +
  'wss://relay.snort.social: recv message response timeout';

beforeEach(() => {
  mocks.invoke.mockReset();
  mocks.isTauri.mockReset();
});

describe('checkForUpdate — dentro do Tauri', () => {
  beforeEach(() => {
    mocks.isTauri.mockReturnValue(true);
  });

  it('actualização disponível: chama o comando do plugin e devolve `available`', async () => {
    mocks.invoke.mockResolvedValue(ACTUALIZACAO);

    const resultado = await checkForUpdate();

    // O nome do comando tem de ser exactamente o que o plugin registou: um
    // `plugin:hashtree-updater|check` mal escrito não é um erro de tipo, é uma
    // rejeição em tempo de execução. Nenhum argumento é passado — o comando não
    // recebe nada (`commands.rs`, `async fn check(app: AppHandle<R>)`).
    expect(mocks.invoke).toHaveBeenCalledTimes(1);
    expect(mocks.invoke).toHaveBeenCalledWith(COMANDO);

    expect(resultado).toEqual({
      kind: 'available',
      update: {
        currentVersion: '0.2.0',
        version: '0.2.1',
        assetName: 'arkhe-ui-app.exe',
        assetKind: 'binary',
        notes: 'liga o `check()` do plugin à interface',
        publishedAt: '2026-09-15T12:00:00Z',
        updateAvailable: true,
      },
    });
  });

  it('já actualizado: `updateAvailable: false` é `current` — o seu próprio estado', async () => {
    mocks.invoke.mockResolvedValue(SEM_NOVIDADE);

    const resultado = await checkForUpdate();

    expect(resultado.kind).toBe('current');
    if (resultado.kind !== 'current') throw new Error('esperava `current`');
    // A comparação existiu: as duas versões ficam disponíveis para a interface
    // as mostrar, em vez de um "está tudo bem" sem números.
    expect(resultado.update.currentVersion).toBe('0.2.0');
    expect(resultado.update.version).toBe('0.2.0');
  });

  it('erro de rede: a rejeição vira `failed`, e NÃO `current`', async () => {
    // É o que o plugin produz hoje: `Err(Error::Resolver(..))`, serializado como
    // string pelo `Serialize for Error` do crate.
    mocks.invoke.mockRejectedValue(ERRO_DE_RELAY);

    const resultado = await checkForUpdate();

    expect(resultado.kind).toBe('failed');
    if (resultado.kind !== 'failed') throw new Error('esperava `failed`');
    expect(resultado.message).toContain('relays');

    // A asserção que a tarefa pede: "não consegui perguntar" não pode ser
    // apresentado como "não há novidade". São estados distintos, e este teste
    // é o que impede que um refactor futuro os funda.
    expect(resultado.kind).not.toBe('current');
    expect(resultado.kind).not.toBe('available');
    expect(resultado.kind).not.toBe('no-release');
  });

  it('um `Error` de verdade também é lido, não só a string do Rust', async () => {
    // O plugin serializa para string, mas um erro de transporte do próprio IPC
    // chega como `Error`. Os dois têm de ser reportados, não convertidos em
    // `[object Object]`.
    mocks.invoke.mockRejectedValue(new Error('IPC fora do ar'));

    const resultado = await checkForUpdate();
    expect(resultado.kind === 'failed' && resultado.message).toBe('IPC fora do ar');
  });

  it('sem asset para o alvo é `failed`, e NÃO `no-release`', async () => {
    // O controlo negativo do CLI, literal: `UpdateError::AssetNotFound`, que o
    // core devolve como `Err` (`error.rs:26` do hashtree-updater). O plugin só
    // converte `ReleaseNotFound`/`ManifestNotFound` em `Ok(None)`, portanto
    // isto chega como rejeição.
    //
    // Este teste existe por causa de uma correcção: a primeira versão do
    // adaptador dava "nenhum asset casou com o alvo" como causa de `Ok(None)`,
    // lido do ramo `updater.rs:109-111` do plugin — que é inalcançável, porque
    // o core devolve sempre `asset: Some(..)` no caminho `Ok`. Chamar-lhe "nada
    // publicado" seria errado; e rotulá-lo "erro de rede" também, que é o que a
    // interface deliberadamente já não faz.
    mocks.invoke.mockRejectedValue('no update asset matched target linux-x86_64');

    const resultado = await checkForUpdate();

    expect(resultado.kind).toBe('failed');
    if (resultado.kind !== 'failed') throw new Error('esperava `failed`');
    expect(resultado.message).toContain('no update asset matched target');
    expect(resultado.kind).not.toBe('no-release');
  });

  it('sem release aplicável: `null` é `no-release`, e não `current`', async () => {
    // `Ok(None)`: `ReleaseNotFound` ou `ManifestNotFound` (`updater.rs:105-106`
    // do plugin). Aqui **não houve** comparação de versões — afirmar "já
    // actualizado" seria afirmar uma medição que não aconteceu.
    mocks.invoke.mockResolvedValue(null);

    const resultado = await checkForUpdate();

    expect(resultado).toEqual({ kind: 'no-release' });
    expect(resultado.kind).not.toBe('current');
  });

  it('uma resposta com forma inesperada é `failed` — nada é adivinhado', async () => {
    mocks.invoke.mockResolvedValue({ version: '0.2.1' });

    const resultado = await checkForUpdate();

    expect(resultado.kind).toBe('failed');
    if (resultado.kind !== 'failed') throw new Error('esperava `failed`');
    expect(resultado.message).toContain('forma declarada');
  });

  it('`notes`/`publishedAt` ausentes são `null`, e não string vazia', async () => {
    const { notes: _n, publishedAt: _p, ...semOpcionais } = ACTUALIZACAO;
    mocks.invoke.mockResolvedValue(semOpcionais);

    const resultado = await checkForUpdate();

    expect(resultado.kind).toBe('available');
    if (resultado.kind !== 'available') throw new Error('esperava `available`');
    expect(resultado.update.notes).toBeNull();
    expect(resultado.update.publishedAt).toBeNull();
  });
});

describe('checkForUpdate — fora do Tauri', () => {
  beforeEach(() => {
    mocks.isTauri.mockReturnValue(false);
  });

  it('devolve `unavailable` e NÃO chama o `invoke`', async () => {
    const resultado = await checkForUpdate();

    expect(resultado).toEqual({ kind: 'unavailable' });
    // A prova que a tarefa pede. O `invoke` desreferencia
    // `window.__TAURI_INTERNALS__` e lançaria no navegador; a guarda é o que
    // impede que a app quebre — e é por isso que este ramo não pode depender de
    // um `try/catch` à volta do `invoke`, que é o que aqui se mede não existir.
    expect(mocks.invoke).not.toHaveBeenCalled();
    // E foi o `isTauri()` que decidiu, não um valor por omissão.
    expect(mocks.isTauri).toHaveBeenCalled();
  });

  it('não lança: o caminho de degradação é um valor, não uma excepção', async () => {
    await expect(checkForUpdate()).resolves.toEqual({ kind: 'unavailable' });
  });

  it('`unavailable` não é `failed` — não há erro nenhum a reportar', async () => {
    const resultado = await checkForUpdate();
    expect(resultado.kind).not.toBe('failed');
  });

  it('`updaterRuntimeAvailable()` reflecte o runtime, sem cache', () => {
    expect(updaterRuntimeAvailable()).toBe(false);

    mocks.isTauri.mockReturnValue(true);
    // Lido em tempo de chamada: se fosse capturado numa constante de módulo,
    // isto continuaria `false`.
    expect(updaterRuntimeAvailable()).toBe(true);
  });
});

describe('parseUpdateInfo', () => {
  it('interpreta a forma declarada', () => {
    expect(parseUpdateInfo(ACTUALIZACAO)).toEqual(ACTUALIZACAO);
  });

  it('recusa o que não é objecto', () => {
    for (const entrada of [null, undefined, 42, 'texto', [], true]) {
      expect(parseUpdateInfo(entrada)).toBeNull();
    }
  });

  it('recusa campos obrigatórios em falta ou com o tipo errado', () => {
    expect(parseUpdateInfo({ ...ACTUALIZACAO, version: 21 })).toBeNull();
    expect(parseUpdateInfo({ ...ACTUALIZACAO, currentVersion: undefined })).toBeNull();
    expect(parseUpdateInfo({ ...ACTUALIZACAO, assetName: null })).toBeNull();
    expect(parseUpdateInfo({ ...ACTUALIZACAO, assetKind: [] })).toBeNull();
    expect(parseUpdateInfo({ ...ACTUALIZACAO, updateAvailable: 'true' })).toBeNull();
  });

  it('recusa um opcional presente mas ilegível, em vez de o engolir', () => {
    expect(parseUpdateInfo({ ...ACTUALIZACAO, notes: 7 })).toBeNull();
    expect(parseUpdateInfo({ ...ACTUALIZACAO, publishedAt: { ano: 2026 } })).toBeNull();
  });

  it('`null` e ausente são a mesma coisa nos opcionais — e não "sem notas"', () => {
    expect(parseUpdateInfo({ ...ACTUALIZACAO, notes: null })?.notes).toBeNull();
    expect(parseUpdateInfo({ ...ACTUALIZACAO, notes: undefined })?.notes).toBeNull();
    // Mas uma nota vazia é uma nota que veio.
    expect(parseUpdateInfo({ ...ACTUALIZACAO, notes: '' })?.notes).toBe('');
  });

  it('`updateAvailable: false` sobrevive à interpretação', () => {
    const interpretado = parseUpdateInfo(SEM_NOVIDADE);
    expect(interpretado).not.toBeNull();
    expect(interpretado?.updateAvailable).toBe(false);
  });
});
