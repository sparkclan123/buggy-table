import type {
  ConnectionInfo,
  Filter,
  FilterMatch,
  IndexInfo,
  QueryResult,
  TableInfo,
} from './types';

const column = (name: string, dataType: string, primaryKey = false, nullable = false) => ({
  name,
  dataType,
  primaryKey,
  nullable,
  editable: !primaryKey,
});
export const sampleTables: TableInfo[] = [
  {
    schema: 'public',
    deletable: true,
    name: 'orders',
    estimatedRows: 450,
    columns: [
      column('id', 'int8', true),
      column('customer', 'text'),
      column('status', 'varchar'),
      column('total', 'numeric'),
      column('country', 'text'),
      column('created_at', 'timestamptz'),
    ],
  },
  {
    schema: 'public',
    deletable: true,
    name: 'customers',
    estimatedRows: 3,
    columns: [
      column('id', 'int8', true),
      column('name', 'text'),
      column('email', 'text', false, true),
      column('country', 'text'),
      column('created_at', 'timestamptz'),
    ],
  },
];

export const sampleConnection: ConnectionInfo = {
  engine: 'postgres',
  id: 'sample',
  name: 'Sample workspace',
  host: 'offline',
  port: 0,
  database: 'sample_store',
  username: 'preview',
  schema: 'public',
  tls: 'localOnly',
  manualEdits: true,
  serverVersion: 'Sample',
  tables: sampleTables,
};

export const sampleRows: Record<string, (string | null)[][]> = {
  orders: Array.from({ length: 450 }, (_, i) => [
    String(i + 1),
    ['Mali S.', 'Arun K.', 'Yuna T.', 'Alex W.'][i % 4],
    ['completed', 'processing', 'pending'][i % 3],
    ((i + 1) * 13.75).toFixed(2),
    ['Thailand', 'Japan', 'Singapore'][i % 3],
    `2026-09-${String((i % 28) + 1).padStart(2, '0')} 10:00:00+00`,
  ]),
  customers: [
    ['1', 'Mali S.', 'mali@example.test', 'Thailand', '2026-09-10 08:00:00+00'],
    ['2', 'Arun K.', 'arun@example.test', 'Thailand', '2026-09-12 10:30:00+00'],
    ['3', 'Yuna T.', null, 'Japan', '2026-09-14 09:15:00+00'],
  ],
};

function matches(value: string | null, filter: Filter, type: string): boolean {
  const comparable = (text: string) =>
    type === 'timestamptz'
      ? Date.parse(text.replace(' ', 'T').replace(/([+-]\d{2})$/, '$1:00'))
      : Number(text);
  switch (filter.operator) {
    case 'isNull':
      return value === null;
    case 'isNotNull':
      return value !== null;
    case 'equals':
      return value !== null && value === filter.value;
    case 'notEquals':
      return value !== null && value !== filter.value;
    case 'contains':
      return value !== null && value.toLowerCase().includes(filter.value.toLowerCase());
    case 'greaterThan':
      return value !== null && comparable(value) > comparable(filter.value);
    case 'lessThan':
      return value !== null && comparable(value) < comparable(filter.value);
  }
}

export function browseSample(
  table: TableInfo,
  filters: Filter[],
  page: number,
  filterMatch: FilterMatch = 'all',
  sortColumn: string | null = null,
  descending = false,
  pageSize = 100,
): QueryResult {
  const rows = (sampleRows[table.name] ?? []).filter((row) => {
    const test = (filter: Filter) => {
      const index = table.columns.findIndex((c) => c.name === filter.column);
      return matches(row[index], filter, table.columns[index]?.dataType ?? '');
    };
    return !filters.length || (filterMatch === 'all' ? filters.every(test) : filters.some(test));
  });
  const order = [
    sortColumn,
    ...table.columns.filter((c) => c.primaryKey).map((c) => c.name),
  ].filter((name, index, all): name is string => name !== null && all.indexOf(name) === index);
  rows.sort((a, b) => {
    for (const name of order) {
      const index = table.columns.findIndex((c) => c.name === name);
      const left = a[index],
        right = b[index];
      if (left === right) continue;
      if (left === null) return 1;
      if (right === null) return -1;
      const type = table.columns[index].dataType;
      const comparison = ['int2', 'int4', 'int8'].includes(type)
        ? BigInt(left) < BigInt(right)
          ? -1
          : 1
        : ['numeric', 'float4', 'float8'].includes(type)
          ? Number(left) - Number(right)
          : left.localeCompare(right);
      if (comparison) return descending ? -comparison : comparison;
    }
    return 0;
  });
  return {
    columns: table.columns.map((c) => c.name),
    rows: rows.slice(page * pageSize, (page + 1) * pageSize),
    elapsedMs: 0,
    truncated: rows.length > (page + 1) * pageSize,
    sql: '',
  };
}

export function sampleIndexes(table: string): IndexInfo[] {
  return [
    {
      name: `${table}_pkey`,
      definition: `CREATE UNIQUE INDEX ${table}_pkey ON public.${table} USING btree (id)`,
      scans: '—',
      size: '—',
      unique: true,
      primary: true,
    },
    ...(table === 'orders'
      ? [
          {
            name: 'orders_status_idx',
            definition: 'CREATE INDEX orders_status_idx ON public.orders USING btree (status)',
            scans: '—',
            size: '—',
            unique: false,
            primary: false,
          },
        ]
      : []),
  ];
}
