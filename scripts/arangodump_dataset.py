"""Synthetic ArangoDB source datasets for the CG-64 dump fixtures.

The same definitions drive two independent things: the generator loads them
into a disposable ArangoDB over HTTP, and `expected()` derives the Native
result the importer must produce from the definitions alone, never from dump
bytes. All values are invented; no real person, company or record appears.
"""

DOCUMENT, EDGE = 2, 3

# Keys cover every punctuation class ArangoDB allows in traditional keys.
CUSTOMERS = [
    {'_key': 'c-1', 'email': 'ana@example.test', 'name': 'Ana Müller', 'tags': ['vip', 'eu'],
     'address': {'city': 'Lisboa', 'geo': [38.7223, -9.1393]}, 'loyalty': {'id': 'L-100'},
     'score': 4.5, 'active': True},
    {'_key': 'c:2', 'email': 'bo@example.test', 'name': '博', 'loyalty': None,
     'nested': {'a': {'b': {'c': {'d': [1, [2, [3]]]}}}}, 'active': False},
    {'_key': 'c@3', 'email': 'cy@example.test', 'name': 'Cy 🚀',
     'numbers': {'big': 9007199254740993, 'negative': -42, 'tenth': 0.1, 'huge': 1e300, 'zero': 0}},
    {'_key': 'c.4', 'email': 'di@example.test', 'empty': {'object': {}, 'array': [], 'string': ''}},
    {'_key': "c_5(x)+,=;$!*'%", 'email': 'ed@example.test', 'loyalty': {'id': 'L-200'},
     'text': 'quote " backslash \\ newline \n tab \t nul-free'},
]

PRODUCTS = [
    {'_key': 'p1', 'sku': 'A-1', 'region': 'eu', 'barcode': '0001', 'category': 'tools',
     'location': [38.7, -9.1], 'expiresAt': 4102444800},
    {'_key': 'p2', 'sku': 'A-1', 'region': 'us', 'barcode': '0002', 'category': 'tools'},
    {'_key': 'p3', 'sku': 'B-7', 'region': 'eu', 'barcode': '0003', 'category': 'garden',
     'description': 'rake with ash handle'},
    {'_key': 'p4', 'sku': 'B-7', 'region': 'apac', 'category': 'garden'},
]

ORDERS = [
    {'_key': 'o1', '_from': 'customers/c-1', '_to': 'products/p1', 'qty': 2, 'at': '2026-01-02'},
    {'_key': 'o2', '_from': 'customers/c:2', '_to': 'products/p3', 'qty': 1, 'at': '2026-01-03'},
    {'_key': 'o3', '_from': "customers/c_5(x)+,=;$!*'%", '_to': 'products/p2', 'qty': 7,
     'at': '2026-02-11'},
]

REFERRALS = [
    {'_key': 'r1', '_from': 'customers/c-1', '_to': 'customers/c@3'},
    {'_key': 'r2', '_from': 'customers/c@3', '_to': 'customers/c.4', 'note': None},
]

# Each index carries its expected Native disposition. `carried` entries become
# enforced unique constraints; everything else is reported, never enforced.
SHOP = {
    'database': 'shop',
    'collections': [
        {'name': 'customers', 'type': DOCUMENT, 'documents': CUSTOMERS, 'indexes': [
            ({'type': 'persistent', 'fields': ['email'], 'unique': True, 'name': 'email_unique'},
             {'carried': {'fields': ['email'], 'sparse': False}}),
            ({'type': 'persistent', 'fields': ['loyalty.id'], 'unique': True, 'sparse': True,
              'name': 'loyalty_unique'},
             {'carried': {'fields': ['loyalty.id'], 'sparse': True}}),
        ]},
        {'name': 'products', 'type': DOCUMENT, 'documents': PRODUCTS,
         'schema': {'rule': {'type': 'object', 'required': ['sku']}, 'level': 'moderate',
                    'message': 'sku required'},
         'indexes': [
             ({'type': 'persistent', 'fields': ['sku', 'region'], 'unique': True, 'name': 'sku_region'},
              {'carried': {'fields': ['sku', 'region'], 'sparse': False}}),
             # `hash` is an alias for `persistent` since ArangoDB 3.9.
             ({'type': 'hash', 'fields': ['barcode'], 'unique': True, 'sparse': True, 'name': 'barcode'},
              {'carried': {'fields': ['barcode'], 'sparse': True}}),
             ({'type': 'persistent', 'fields': ['category'], 'name': 'by_category'},
              {'not_carried': 'non_unique_index'}),
             ({'type': 'ttl', 'fields': ['expiresAt'], 'expireAfter': 3600, 'name': 'expiry'},
              {'not_carried': 'unsupported_index_type'}),
             ({'type': 'geo', 'fields': ['location'], 'geoJson': False, 'name': 'where'},
              {'not_carried': 'unsupported_index_type'}),
             ({'type': 'inverted', 'fields': ['description'], 'name': 'search_description'},
              {'not_carried': 'unsupported_index_type'}),
         ]},
        {'name': 'orders', 'type': EDGE, 'documents': ORDERS, 'indexes': [
            ({'type': 'persistent', 'fields': ['_from', '_to', 'at'], 'unique': True,
              'name': 'one_order_per_day'},
             {'not_carried': 'edge_unique_unsupported'}),
        ]},
        {'name': 'referrals', 'type': EDGE, 'documents': REFERRALS, 'indexes': []},
        {'name': 'empty_docs', 'type': DOCUMENT, 'documents': [], 'indexes': []},
        {'name': 'empty_edges', 'type': EDGE, 'documents': [], 'indexes': []},
    ],
    'views': [{'name': 'customer_search', 'type': 'arangosearch',
               'links': {'customers': {'fields': {'name': {}}}}}],
    'graphs': [{'name': 'shop_graph', 'edgeDefinitions': [
        {'collection': 'orders', 'from': ['customers'], 'to': ['products']}]}],
}

# Every problem ArangoDB itself lets a user create but V1 must refuse.
BROKEN = {
    'database': 'broken',
    'collections': [
        {'name': 'parts', 'type': DOCUMENT, 'documents': [{'_key': 'x1'}], 'indexes': []},
        {'name': 'links', 'type': EDGE, 'indexes': [], 'documents': [
            {'_key': 'l1', '_from': 'parts/x1', '_to': 'parts/missing'},
            {'_key': 'l2', '_from': 'ghost/g1', '_to': 'parts/x1'},
        ]},
        {'name': 'order-lines', 'type': DOCUMENT, 'documents': [{'_key': 'ol1'}], 'indexes': []},
        {'name': 'facts', 'type': EDGE, 'documents': [], 'indexes': []},
        {'name': 'count', 'type': DOCUMENT, 'documents': [{'_key': 'n1'}], 'indexes': []},
    ],
    'views': [],
    'graphs': [],
}

BROKEN_ERRORS = [
    {'code': 'unresolved_edge', 'collection': 'links', 'key': 'l1'},
    {'code': 'unresolved_edge', 'collection': 'links', 'key': 'l2'},
    {'code': 'incompatible_collection_name', 'collection': 'order-lines'},
    {'code': 'reserved_collection_name', 'collection': 'facts'},
    {'code': 'reserved_collection_name', 'collection': 'count'},
]

DATASETS = {'shop': SHOP, 'broken': BROKEN}


def expected(dataset):
    """The Native result implied by the definitions: exact documents without
    `_id`/`_rev`, collection types, carried constraints and every reported
    non-carried item."""
    collections, constraints, not_carried = {}, [], []
    for collection in dataset['collections']:
        name = collection['name']
        collections[name] = {
            'type': 'edge' if collection['type'] == EDGE else 'document',
            'documents': {doc['_key']: doc for doc in collection['documents']},
        }
        for definition, disposition in collection['indexes']:
            if 'carried' in disposition:
                constraints.append({'collection': name, 'unique': True, **disposition['carried']})
            else:
                not_carried.append({'collection': name, 'kind': 'index',
                                    'detail': definition['name'],
                                    'reason': disposition['not_carried']})
        if 'schema' in collection:
            not_carried.append({'collection': name, 'kind': 'schema', 'detail': 'validation rule',
                                'reason': 'unsupported_metadata'})
    for view in dataset['views']:
        not_carried.append({'collection': None, 'kind': 'view', 'detail': view['name'],
                            'reason': 'unsupported_metadata'})
    return {'database': dataset['database'], 'collections': collections,
            'constraints': constraints, 'not_carried': not_carried}
