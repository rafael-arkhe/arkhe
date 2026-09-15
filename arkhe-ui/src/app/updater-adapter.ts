/**
 * A fronteira com o **plugin de actualização** (`tauri-plugin-hashtree-updater`
 * 0.2.52) — o único módulo da app que sabe que existe um resolvedor de releases
 * do outro lado, e o único que sabe o que a resposta significa. O resto da app
 * fala só com `checkForUpdate`.
 *
 * # Porque é um adaptador próprio, ao lado do `gguf-adapter` e não dentro dele
 *
 * São as duas fronteiras com o **processo Rust**, e partilham o idioma — união
 * discriminada em vez de `null`s que se confundem — mas não partilham nada
 * mais:
 *
 * 1. **Outro dono.** O `gguf-adapter` fala com um comando *desta app*
 *    (`inspect_gguf_model`, registado em `src-tauri/src/lib.rs`). Aqui fala-se
 *    com um comando de um **plugin de terceiros**, cuja superfície é fixada
 *    fora deste repositório. Misturá-los faria com que subir a versão do plugin
 *    implicasse mexer no módulo que descreve o comando local.
 * 2. **O que está em jogo é outro.** Ali lê-se um ficheiro do disco, com os
 *    privilégios do utilizador, e o pior caso é uma leitura falhada. Aqui
 *    resolve-se um manifesto publicado por terceiros e, na continuação desta
 *    API, substitui-se o binário em execução — a interpretação da resposta tem
 *    de ser estrita pelo mesmo motivo, mas o estrago de a aceitar mal é maior.
 * 3. **Não há eco nem entrada.** O `inspect_gguf_model` recebe um caminho e
 *    ecoa-o; este não recebe nada e responde com factos sobre o release.
 *
 * # Os cinco estados, e porque nenhum deles se colapsa
 *
 * O `check()` do plugin tem três desfechos distintos (`src/updater.rs` do
 * crate), e esta app recusa-se a fundi-los:
 *
 * | resposta do plugin | causa no Rust | estado aqui |
 * |---|---|---|
 * | `Ok(Some(..))`, `updateAvailable: true` | há versão mais recente para este alvo | `available` |
 * | `Ok(Some(..))`, `updateAvailable: false` | houve comparação, e a publicada não é mais recente | `current` |
 * | `Ok(None)` | `ReleaseNotFound`/`ManifestNotFound` (`updater.rs:105-106`) — nada publicado, ou sem manifesto no caminho | `no-release` |
 * | `Err(..)` | rede/relays (`Resolve`), nenhum asset para o alvo (`AssetNotFound`), manifesto inválido, configuração | `failed` |
 * | (não se chama) | não há runtime Tauri | `unavailable` |
 *
 * O caso que interessa separar é o terceiro: o `Ok(None)` é a única resposta em
 * que **não houve comparação de versões nenhuma**, e por isso não pode ser
 * apresentado como "já está actualizado" — que é uma afirmação sobre uma
 * comparação. O README do plugin recomenda tratá-lo como "sem novidade" e ficar
 * calado; é bom conselho para um banner automático e insuficiente para uma
 * consulta que o utilizador pediu.
 *
 * # Uma correcção que veio do código, e não do README
 *
 * A primeira versão deste ficheiro dava três causas ao `Ok(None)`, incluindo
 * "nenhum asset casou com o alvo" — lido do ramo `let Some(asset) = ..else {
 * return Ok(None) }` do plugin (`updater.rs:109-111`). **Esse ramo é
 * inalcançável.** No core 0.2.85, `check()` obtém o asset por
 * `select_asset(..).ok_or_else(AssetNotFound)?` e devolve sempre
 * `asset: Some(..)` no caminho `Ok` (core `updater.rs:118-127`); o plugin só
 * converte `ReleaseNotFound` e `ManifestNotFound` em `Ok(None)`. Logo um alvo
 * sem asset chega como **`Err(AssetNotFound)`** — o mesmo erro que o controlo
 * negativo do CLI produz ("no update asset matched target linux-x86_64", core
 * `error.rs:26`) — e cai em `failed`, não em `no-release`.
 *
 * É por isso que `failed` **não** se chama "erro de rede" na interface: o mesmo
 * caminho de rejeição transporta causas que não são de rede, e rotulá-las todas
 * como rede seria a mesma fusão que este ficheiro recusa, ao contrário. A causa
 * vem do plugin, em texto, e é mostrada como veio.
 *
 * E `failed` nunca é `current`: com os relays em baixo o `check()` rejeita, e
 * uma rejeição apresentada como "não há novidade" é a única resposta que este
 * ecrã não pode dar — é a diferença entre "confirmei que está tudo em ordem" e
 * "não consegui perguntar".
 *
 * # O que este módulo **não** faz
 *
 * Não descarrega nem instala. O `downloadAndInstall` existe no glue copiado
 * (`hashtree-updater-guest.ts`) mas não tem interface nem teste nesta fase:
 * substituir o binário em execução é uma acção destrutiva que merece um
 * controlo próprio, com confirmação e progresso, e não um efeito secundário de
 * um botão de consulta.
 */
import { isTauri } from '@tauri-apps/api/core';

import { check, type Update } from './hashtree-updater-guest';

/**
 * Os factos que a interface mostra, já estreitados — a forma declarada.
 *
 * `notes` e `publishedAt` são `Option<String>` do lado do Rust e opcionais no
 * `UpdateMetadata` do glue: aqui ficam `null` quando ausentes, e `null` é
 * "ausente", não `''`. Uma nota vazia e uma nota que não veio são factos
 * diferentes para quem lê.
 */
export interface UpdateInfo {
  /** A versão de `package_info()` — lida de `tauri.conf.json`, não do binário. */
  currentVersion: string;
  /** A versão publicada que o manifesto declara. */
  version: string;
  assetName: string;
  /** `binary`, `app-bundle`, `appimage`, … — o que o alvo escolheu. */
  assetKind: string;
  notes: string | null;
  publishedAt: string | null;
  updateAvailable: boolean;
}

/**
 * O que pode acontecer ao consultar — cinco casos, não dois.
 *
 * Os três primeiros (`available`, `current`, `no-release`) e o quarto
 * (`failed`) são **estados distintos**, com textos distintos: colapsar "não
 * consegui perguntar" em "não há novidade" é a única coisa que este ecrã não
 * pode fazer (ver o cabeçalho). `unavailable` é o quinto, e é o caso normal de
 * um `dist-app` servido no navegador — não um erro.
 */
export type UpdateCheck =
  | { kind: 'available'; update: UpdateInfo }
  | { kind: 'current'; update: UpdateInfo }
  | { kind: 'no-release' }
  | { kind: 'unavailable' }
  | { kind: 'failed'; message: string };

/**
 * Há um runtime Tauri a atender o IPC?
 *
 * Delega no `isTauri()` do `@tauri-apps/api`, que lê `globalThis.isTauri` —
 * a flag que o Tauri v2 injecta no webview. Leitura em tempo de chamada e não
 * constante de módulo, pela mesma razão que no `gguf-adapter`: um valor
 * capturado no import seria uma resposta a uma pergunta feita antes de a página
 * estar montada.
 */
export function updaterRuntimeAvailable(): boolean {
  return isTauri();
}

/**
 * `undefined` para ausente, string para presente, `null` para **presente e
 * ilegível**.
 *
 * O terceiro caso é o que obriga a recusar a resposta inteira em vez de a
 * engolir: um `notes` que veio com o tipo errado é uma resposta que não se
 * interpreta, e apresentar `null` como se fosse "sem notas" seria inventar um
 * facto a partir de um campo que não se soube ler. É o mesmo critério do
 * `parseInspection` do [`gguf-adapter`](./gguf-adapter.ts).
 */
function optionalString(value: unknown): string | null | undefined {
  if (value === undefined || value === null) return undefined;
  return typeof value === 'string' ? value : null;
}

/**
 * Estreita a resposta do plugin para a forma declarada — ou recusa-a.
 *
 * Aqui não há valor conservador para onde cair, ao contrário do relatório de
 * atestação (onde um campo ilegível vira `false`, que é o lado seguro de
 * "não verificado"). Não existe `version` que signifique "não sei": aceitar
 * uma resposta malformada e mostrar campos `undefined` na interface seria
 * apresentar lixo como se fosse medido. Por isso devolve `null` — e quem chama
 * reporta que a forma não é a declarada.
 */
export function parseUpdateInfo(value: unknown): UpdateInfo | null {
  if (typeof value !== 'object' || value === null) return null;

  const raw = value as Record<string, unknown>;

  const currentVersion = typeof raw.currentVersion === 'string' ? raw.currentVersion : null;
  const version = typeof raw.version === 'string' ? raw.version : null;
  const assetName = typeof raw.assetName === 'string' ? raw.assetName : null;
  const assetKind = typeof raw.assetKind === 'string' ? raw.assetKind : null;
  const updateAvailable = typeof raw.updateAvailable === 'boolean' ? raw.updateAvailable : null;

  const notes = optionalString(raw.notes);
  const publishedAt = optionalString(raw.publishedAt);

  if (
    currentVersion === null ||
    version === null ||
    assetName === null ||
    assetKind === null ||
    updateAvailable === null ||
    notes === null ||
    publishedAt === null
  ) {
    return null;
  }

  return {
    currentVersion,
    version,
    assetName,
    assetKind,
    notes: notes ?? null,
    publishedAt: publishedAt ?? null,
    updateAvailable,
  };
}

/**
 * Pergunta ao plugin se há versão mais recente para esta plataforma.
 *
 * **Nunca lança.** As cinco saídas estão em [`UpdateCheck`], para que o
 * chamador tenha de olhar para todas em vez de apanhar uma excepção genérica.
 *
 * Fora do Tauri devolve `unavailable` **sem chamar o `invoke`** — o `invoke`
 * desreferencia `window.__TAURI_INTERNALS__` e lançaria. A guarda não é
 * defensiva a mais: um `dist-app` servido no navegador é um alvo suportado
 * desta app, não um erro.
 */
export async function checkForUpdate(): Promise<UpdateCheck> {
  if (!isTauri()) return { kind: 'unavailable' };

  let resposta: Update | null;
  try {
    resposta = await check();
  } catch (error: unknown) {
    // O `Err(..)` do plugin serializa como **string** (`Serialize for Error`
    // em `error.rs` do crate), e é isso que a promessa rejeitada carrega — não
    // um `Error`. É por aqui que passa, hoje, o caso dos relays que não
    // respondem à query de resolução da árvore.
    return { kind: 'failed', message: error instanceof Error ? error.message : String(error) };
  }

  // `Ok(None)`: nada publicado, ou sem manifesto no caminho declarado. É a
  // única resposta sem comparação de versões — ver a tabela do cabeçalho.
  if (resposta === null) return { kind: 'no-release' };

  const update = parseUpdateInfo(resposta);
  if (update === null) {
    return {
      kind: 'failed',
      message:
        'a resposta de `plugin:hashtree-updater|check` não tem a forma declarada — não é interpretada em vez de adivinhada',
    };
  }

  // A comparação existiu e foi o plugin que a fez: `updateAvailable` só é
  // `false` quando houve manifesto e asset selecionado, e a versão publicada
  // não é mais recente do que a corrente.
  return update.updateAvailable
    ? { kind: 'available', update }
    : { kind: 'current', update };
}
