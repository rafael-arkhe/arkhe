/**
 * Cópia literal do `guest-js/index.ts` do crate
 * **`tauri-plugin-hashtree-updater` 0.2.52**, em
 * `$CARGO_HOME/registry/src/<registro>/tauri-plugin-hashtree-updater-0.2.52/guest-js/index.ts`.
 *
 * (O caminho fica em `$CARGO_HOME` com o directório do registo elidido, de
 * propósito: um caminho absoluto com o nome do utilizador dentro seria embutido
 * no bundle da app e, por ele, no binário — é o que a varredura de sanitização
 * do relatório da Fase 5 procura.)
 *
 * # Porque é uma cópia, e não uma dependência
 *
 * O plugin **não publica pacote npm**. Medido no registo, não presumido:
 * `npm view <nome> version` devolve 404 em todos os nomes plausíveis —
 * `@hashtree/tauri-plugin-hashtree-updater`, `tauri-plugin-hashtree-updater`,
 * `@hashtree/updater`, `@tauri-apps/plugin-hashtree-updater` e
 * `hashtree-updater`. Não há `import` possível; a única fonte do glue é este
 * ficheiro dentro do crate. O README do próprio plugin diz o mesmo, nas
 * palavras dele: *"There is no published npm package — copy `guest-js/index.ts`
 * from this crate into your app"*.
 *
 * # Porque está aqui, e não dentro do adaptador
 *
 * Esta cópia mantém-se **literal**, e é isso que a torna útil: o diff contra o
 * crate é zero, e subir o plugin de versão é um diff legível em vez de uma
 * reinterpretação. Tudo o que é decisão *desta* app — a guarda de runtime, os
 * estados como união discriminada, a recusa de formas inesperadas — vive no
 * [`updater-adapter`](./updater-adapter.ts). Aqui está só o que o plugin
 * escreveu: o nome do comando e a forma `camelCase` da resposta.
 *
 * `downloadAndInstall` e o `Channel` vêm na cópia porque a cópia é integral.
 * Nesta fase **não** estão ligados a interface nenhuma — só o `check()` está.
 * O plugin trata de descarregar e de substituir o binário em disco, e nada
 * nesta app o pede; a superfície fica intacta para que uma fase que a queira
 * usar não tenha de voltar a copiar o ficheiro.
 */
import { invoke, Channel } from '@tauri-apps/api/core'

export interface UpdateMetadata {
  currentVersion: string
  version: string
  assetName: string
  assetKind: string
  notes?: string
  publishedAt?: string
  updateAvailable: boolean
}

export type DownloadEvent =
  | { event: 'started'; data: { contentLength?: number } }
  | { event: 'progress'; data: { chunkLength: number; downloaded: number } }
  | { event: 'finished'; data: { total: number } }

export interface InstallOptions {
  /** Override the install destination provided in `tauri.conf.json`. */
  destination?: string
  /** Override the asset kind reported by the manifest. */
  kind?: string
  /** Set the unix executable bit after install (binary kind only). */
  executable?: boolean
}

export class Update {
  readonly currentVersion: string
  readonly version: string
  readonly assetName: string
  readonly assetKind: string
  readonly notes?: string
  readonly publishedAt?: string
  readonly updateAvailable: boolean

  constructor(meta: UpdateMetadata) {
    this.currentVersion = meta.currentVersion
    this.version = meta.version
    this.assetName = meta.assetName
    this.assetKind = meta.assetKind
    this.notes = meta.notes
    this.publishedAt = meta.publishedAt
    this.updateAvailable = meta.updateAvailable
  }

  /**
   * Download the matching asset and install it via the platform-specific
   * dispatcher (binary swap, .app bundle on macOS, AppImage on Linux).
   */
  async downloadAndInstall(
    onEvent?: (event: DownloadEvent) => void,
    options?: InstallOptions
  ): Promise<UpdateMetadata> {
    const channel = new Channel<DownloadEvent>()
    if (onEvent) channel.onmessage = onEvent
    return await invoke<UpdateMetadata>(
      'plugin:hashtree-updater|download_and_install',
      { onEvent: channel, ...options }
    )
  }
}

/** Resolve the manifest, returning `null` if no asset matches the platform. */
export async function check(): Promise<Update | null> {
  const meta = await invoke<UpdateMetadata | null>(
    'plugin:hashtree-updater|check'
  )
  return meta ? new Update(meta) : null
}
