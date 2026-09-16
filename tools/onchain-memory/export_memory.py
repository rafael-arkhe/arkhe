import json, pathlib, collections, sys

SRC = pathlib.Path('arkhe-monorepo/etherscan_verified_signatures_export.json')
OUT = pathlib.Path('docs/onchain-memory')

d = json.loads(SRC.read_text(encoding='utf-8', errors='replace'))
regs = [r for r in d if isinstance(r, dict) and r.get('date')]

# ordenar cronologicamente -- a memoria le-se do principio
regs.sort(key=lambda r: r.get('date', ''))

comprimentos = [len(r.get('msg') or '') for r in regs]
print('  mensagens:', len(regs))
print('  comprimento: min', min(comprimentos), 'max', max(comprimentos), 'soma', sum(comprimentos))
print('  truncadas no fim com "...":', sum(1 for r in regs if (r.get('msg') or '').rstrip().endswith('...')))

OUT.mkdir(parents=True, exist_ok=True)

por_ano = collections.defaultdict(list)
for r in regs:
    por_ano[r['date'][:4]].append(r)

temas = ['arkhe','cathedral','lean','rust','gguf','rekor','sigstore','manifold','wormgraph',
         'evidence','pqc','merkle','coherence','topolog','wasm','tokio','quantum']
cont = collections.Counter()
for r in regs:
    m = (r.get('msg') or '').lower()
    for t in temas:
        if t in m:
            cont[t] += 1

# --- indice ---
L = []
L.append('# On-chain memory of the Arkhe project\n')
L.append('Every message below was signed on-chain by the project wallet and is')
L.append('therefore timestamped by the Ethereum blockchain, not by us. The record')
L.append('runs from **%s** to **%s**.' % (regs[0]['date'], regs[-1]['date']))
L.append('')
L.append('| | |')
L.append('|:--|:--|')
L.append('| Messages | %d |' % len(regs))
L.append('| Period | %s .. %s |' % (regs[0]['date'][:10], regs[-1]['date'][:10]))
L.append('| Years | %s |' % ', '.join(sorted(por_ano)))
L.append('| Text size | %d characters |' % sum(comprimentos))
L.append('')
L.append('The source is `arkhe-monorepo/etherscan_verified_signatures_export.json`,')
L.append('collected from the Etherscan verified-signatures feed. `tools/onchain-memory/`')
L.append('regenerates this directory from it.')
L.append('')
L.append('## Topics, by number of messages that mention them\n')
L.append('| Topic | Messages |')
L.append('|:--|--:|')
for t, n in cont.most_common():
    L.append('| %s | %d |' % (t, n))
L.append('')
L.append('## By year\n')
for ano in sorted(por_ano):
    L.append('- [%s](%s.md) -- %d messages' % (ano, ano, len(por_ano[ano])))
L.append('')
L.append('## The first message\n')
L.append('The record opens with the wallet verification that established the address:')
L.append('')
L.append('> ' + (regs[0].get('msg') or '').replace('\n', '\n> '))
L.append('')
(OUT / 'README.md').write_text('\n'.join(L), encoding='utf-8')
print('  README.md:', (OUT / 'README.md').stat().st_size, 'bytes')

# --- um ficheiro por ano, texto completo ---
for ano in sorted(por_ano):
    rs = por_ano[ano]
    L = ['# %s -- on-chain messages\n' % ano]
    L.append('%d messages, in chronological order. Text as signed; nothing shortened.\n' % len(rs))
    for r in rs:
        L.append('---\n')
        L.append('### `%s` -- id %s\n' % (r.get('date'), r.get('id')))
        L.append((r.get('msg') or '').rstrip())
        L.append('')
    (OUT / ('%s.md' % ano)).write_text('\n'.join(L), encoding='utf-8')
    print('  %s.md: %d bytes (%d mensagens)' % (ano, (OUT / ('%s.md' % ano)).stat().st_size, len(rs)))

print()
print('  --- o que fica proibido por engano: verificar tamanho total ---')
tot = sum((OUT / f).stat().st_size for f in ['README.md'] + ['%s.md' % a for a in por_ano])
print('  total do diretorio:', tot, 'bytes')