import csv

rows = list(csv.DictReader(open('sweep_full.csv', newline='')))
for r in rows:
    r['peak_drift'] = float(r['peak_drift'])
    r['branches'] = int(r['branches'])

print('rows:', len(rows))

print('\n[A] minimize peak_drift with branches >= 4:')
cand = [r for r in rows if r['branches'] >= 4]
cand.sort(key=lambda r: r['peak_drift'])
for r in cand[:5]:
    print('  drift=%.4f branches=%2d  seed=%2d th=%.2f n=%2d t_max=%.1f dt=%.3f ref=%s' % (
        r['peak_drift'], r['branches'], int(r['seed']), float(r['threshold']),
        int(r['n']), float(r['t_max']), float(r['dt']), r['reference']))

print('\n[B] maximize branches with peak_drift < 0.02:')
cand = [r for r in rows if r['peak_drift'] < 0.02]
cand.sort(key=lambda r: (-r['branches'], r['peak_drift']))
for r in cand[:5]:
    print('  drift=%.4f branches=%2d  seed=%2d th=%.2f n=%2d t_max=%.1f dt=%.3f ref=%s' % (
        r['peak_drift'], r['branches'], int(r['seed']), float(r['threshold']),
        int(r['n']), float(r['t_max']), float(r['dt']), r['reference']))

print('\n[C] joint: best drift among configs that ALSO pass passed_i16 & passed_i18 flags:')
cand = [r for r in rows if r['passed_i16'] == 'true' and r['passed_i18'] == 'true']
cand.sort(key=lambda r: r['peak_drift'])
print('  count =', len(cand))
for r in cand[:5]:
    print('  drift=%.4f branches=%2d  seed=%2d th=%.2f n=%2d t_max=%.1f dt=%.3f ref=%s' % (
        r['peak_drift'], r['branches'], int(r['seed']), float(r['threshold']),
        int(r['n']), float(r['t_max']), float(r['dt']), r['reference']))
