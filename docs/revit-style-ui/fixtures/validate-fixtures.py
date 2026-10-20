"""Documentation-fixture checks only; not a Dynamo/OpenBIM engine or full schema.

Run: python docs/revit-style-ui/fixtures/validate-fixtures.py
No imports/installs/evaluation of graph Python source. Binary64 edge formatting,
actual model commits/security enforcement and Dynamo oracle execution are not tested.
"""
import copy
import json
import math
from pathlib import Path

ROOT = Path(__file__).resolve().parent


def load(name):
    def unique(pairs):
        result = {}
        for key, value in pairs:
            assert key not in result, f'duplicate JSON key {key}'
            result[key] = value
        return result
    return json.loads((ROOT / name).read_text(encoding='utf-8'),
                      object_pairs_hook=unique,
                      parse_constant=lambda s: (_ for _ in ()).throw(ValueError(s)))


def canonical(value):
    """Fixture numeric subset: integer JSON numbers only, SI decimal strings."""
    def walk(v, path=()):
        if isinstance(v, dict):
            d = {k: walk(x, path + (k,)) for k, x in v.items()}
            # Sort only declared schema paths, never dictionary/list values that
            # happen to use the same field names as graph collections.
            unordered = ()
            if not path:
                unordered = ('nodes', 'connectors', 'inputs', 'outputs', 'dependencies')
            elif path == ('view',):
                unordered = ('nodes', 'groups', 'notes', 'reroutes')
            for key in unordered:
                if key in d:
                    d[key].sort(key=lambda x: x['id'])
            if path == ('nodes', '*'):
                d['ports'].sort(key=lambda x: (x['direction'], x['index']))
                if 'call' in d:
                    indices = {p['id']: p['index'] for p in d['ports']}
                    d['call']['arguments'].sort(key=lambda x: indices[x['port_id']])
            if path == ('python', 'environment_lock'):
                d['artifacts'].sort(key=lambda x: (x['name'], x['version'], x['origin']))
            return d
        if isinstance(v, list):
            return [walk(x, path + ('*',)) for x in v]
        assert not isinstance(v, float), 'binary64 edge canonicalization not covered'
        return v
    return (json.dumps(walk(value), ensure_ascii=False, allow_nan=False,
                       sort_keys=True, indent=2) + '\n').encode('utf-8')


def validate_graph(g):
    assert g['schema_version'] == 1
    assert g['replication_profile'] == 'dynamo-evidence-v1'
    assert g['view']['run_mode'] in ('Manual', 'Automatic')
    for name in ('nodes', 'connectors', 'inputs', 'outputs', 'dependencies'):
        ids = [x['id'] for x in g[name]]
        assert all(isinstance(x, str) and x for x in ids)
        assert len(ids) == len(set(ids)), name
    ports = {}
    nodes = {n['id']: n for n in g['nodes']}
    for n in nodes.values():
        assert n['lacing'] in ('Auto', 'Shortest', 'Longest', 'CrossProduct', 'Disabled')
        seen = set()
        for p in n['ports']:
            assert p['id'] not in ports
            ports[p['id']] = (n['id'], p)
            assert (p['direction'], p['index']) not in seen
            seen.add((p['direction'], p['index']))
            assert p['direction'] in ('input', 'output')
            assert p['rank'] == 'any' or isinstance(p['rank'], int) and p['rank'] >= 0
            assert isinstance(p['level'], int) and p['level'] >= 1
            assert not p['keep_structure'] or p['use_levels']
            if p['direction'] == 'output':
                assert not p['use_levels'] and not p['keep_structure']
        if 'call' in n:
            assert n['call']['guide_source'] in ('lacing', 'explicit')
            for arg in n['call']['arguments']:
                owner, p = ports[arg['port_id']]
                assert owner == n['id'] and p['direction'] == 'input'
                for guide in arg['guides']:
                    assert type(guide['value']) is int and type(guide['longest']) is bool
        managed = n.get('extensions', {}).get('openbim', {}).get('managed_output')
        if managed:
            assert managed['mode'] in ('update_previous', 'always_create_new', 'detach')
            assert managed['identity']['kind'] in ('external_key', 'position')
            assert managed['reorder'] in ('stable_keys', 'reject_changed_order')
            assert managed['shrink'] in ('retain', 'propose_delete')
            assert managed['detach'] == 'retain_elements_tombstone'
            assert managed['create_new'] == 'retain_generations'
            if managed['identity']['kind'] == 'position':
                assert managed['reorder'] == 'reject_changed_order'
    def endpoint(e, direction):
        owner, p = ports[e['port_id']]
        assert owner == e['node_id'] and p['direction'] == direction
        return p
    incoming = set()
    edges = {n: [] for n in nodes}
    for c in g['connectors']:
        source = endpoint(c['start'], 'output')
        target = endpoint(c['end'], 'input')
        assert source['type'] == target['type'] and source['rank'] == target['rank']
        assert target['id'] not in incoming
        incoming.add(target['id'])
        edges[c['start']['node_id']].append(c['end']['node_id'])
    def visit(n, active, done):
        assert n not in active, 'cycle'
        if n not in done:
            for e in edges[n]:
                visit(e, active | {n}, done)
            done.add(n)
    done = set()
    for n in nodes:
        visit(n, set(), done)
    for key, direction in (('inputs', 'input'), ('outputs', 'output')):
        orders, bindings = set(), set()
        for i in g[key]:
            p = endpoint(i['binding'], direction)
            assert i['type'] == p['type'] and i['rank'] == p['rank']
            assert i['order'] not in orders and p['id'] not in bindings
            orders.add(i['order']); bindings.add(p['id'])
    def values(v):
        if isinstance(v, dict):
            if v.get('kind') == 'quantity':
                import re
                assert re.fullmatch(r'-?(?:0|[1-9][0-9]*)(?:\.[0-9]*[1-9])?', v['si'])
                assert v['si'] != '-0'
                assert len(v['dimension']) == 7 and all(type(x) is int for x in v['dimension'])
                assert v['display_unit'] == 'm' and v['dimension'] == [1, 0, 0, 0, 0, 0, 0]
            for x in v.values(): values(x)
        elif isinstance(v, list):
            for x in v: values(x)
        elif isinstance(v, float): assert math.isfinite(v)
    values(g)
    policy = g['extensions']['openbim']
    assert policy['permissions']['model'] in ('read_only', 'propose_own_model')
    assert policy['trust']['commit_confirmation'] == 'per_run_human'
    assert policy['trust']['python_policy'] == 'trusted_local_only'
    assert policy['trust']['install_approval'] == 'separate_locked_artifacts'
    assert policy['cache']['mode'] == 'model_identity_snapshot_generation'
    assert policy['cache']['python'] == policy['cache']['network'] == 'uncached'
    py = g['python']
    assert all(py[k] for k in ('engine', 'version', 'build', 'abi', 'client_api', 'environment_lock'))
    assert py['environment_lock']['isolation'] == 'venv'
    for a in py['environment_lock']['artifacts']:
        assert a['origin'].endswith('.whl')
        assert len(a['sha256']) == 64 and all(c in '0123456789abcdef' for c in a['sha256'])


def reference_case(c):
    """Small source-derived arithmetic checks, deliberately not a generic engine."""
    if c['operation'] == 'identity_any_rank':
        def flatten(x):
            return sum((flatten(y) for y in x), []) if isinstance(x, list) else [x]
        return flatten(c['inputs'][0])
    if c['operation'] == 'sum_rank_one':
        return [[sum(xs) for xs in group] for group in c['inputs'][0]]
    a, b = c['inputs']
    if not isinstance(a, list): return [a + x for x in b]
    if c.get('guides') == [[2], [1]]:
        return [[x + y for x in a] for y in b]
    if c['lacing'] == 'CrossProduct':
        return [[x + y for y in b] for x in a]
    if c['lacing'] == 'Longest':
        return [a[min(i, len(a)-1)] + b[min(i, len(b)-1)] for i in range(max(len(a), len(b)))]
    return [x + y for x, y in zip(a, b)]


def main():
    corpus = load('replication-v1.json')
    ids = [c['id'] for c in corpus['cases'] + corpus['gated_cases']]
    assert len(ids) == len(set(ids))
    for c in corpus['cases']:
        assert reference_case(c) == c['expected'], c['id']
        assert c['evidence'].startswith('https://')
    for c in corpus['gated_cases']:
        assert c['dynamo_expected'] is None
        assert c['expected_diagnostic'] == 'UNRESOLVED_REPLICATION_PROFILE'
    graph = load('graph-v1.json')
    validate_graph(graph)
    expected = canonical(graph)
    actual = (ROOT / 'graph-v1.json').read_bytes().replace(b'\r\n', b'\n')
    assert expected == actual
    assert canonical(json.loads(expected)) == expected
    changed = copy.deepcopy(graph)
    for key in ('nodes', 'connectors', 'inputs', 'outputs', 'dependencies'):
        changed[key].reverse()
    for n in changed['nodes']: n['ports'].reverse()
    assert canonical(changed) == expected
    changed['inputs'][0]['default']['dimension'].reverse()
    assert canonical(changed) != expected, 'semantic value ordering must remain significant'
    named_lists = copy.deepcopy(graph)
    node = next(n for n in named_lists['nodes'] if n['id'] == 'n-size')
    node['parameters']['literal_dictionary'] = {'inputs': [{'id': 'z'}, {'id': 'a'}]}
    parsed = json.loads(canonical(named_lists))
    saved_node = next(n for n in parsed['nodes'] if n['id'] == 'n-size')
    assert saved_node['parameters']['literal_dictionary']['inputs'] == [{'id': 'z'}, {'id': 'a'}]
    node['call']['arguments'][0]['guides'] = [{'value': 2, 'longest': False}, {'value': 1, 'longest': False}]
    parsed = json.loads(canonical(named_lists))
    saved_node = next(n for n in parsed['nodes'] if n['id'] == 'n-size')
    assert [g['value'] for g in saved_node['call']['arguments'][0]['guides']] == [2, 1]
    # Negative structural/policy cases; this is not exhaustive schema validation.
    negatives = []
    def reject(edit):
        g = copy.deepcopy(graph); edit(g); negatives.append(g)
    reject(lambda g: g['nodes'].append(copy.deepcopy(g['nodes'][0])))
    reject(lambda g: g['connectors'][0]['end'].update(port_id='missing'))
    reject(lambda g: g['connectors'][0].update(start=copy.deepcopy(g['connectors'][0]['end'])))
    reject(lambda g: g.update(schema_version=2))
    reject(lambda g: g['extensions']['openbim']['trust'].update(commit_confirmation='automatic'))
    reject(lambda g: g['inputs'][1].update(binding=copy.deepcopy(g['inputs'][0]['binding'])))
    reject(lambda g: g['inputs'][0]['default'].update(si='NaN'))
    reject(lambda g: g['inputs'][0]['default'].update(si='-0'))
    for g in negatives:
        try: validate_graph(g)
        except (AssertionError, KeyError, ValueError): pass
        else: raise AssertionError('negative fixture was accepted')
    print(f'PASS: {len(corpus["cases"])} source-derived expected-output checks; '
          f'{len(corpus["gated_cases"])} explicit unresolved gates; '
          f'graph canonical round-trip/order checks; {len(negatives)} rejection checks')
    print('NOT TESTED: Dynamo runtime, production engine/commit/security behavior, full schema or binary64 edge conformance')


if __name__ == '__main__':
    main()
