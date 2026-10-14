-- Disposable local test database ONLY. Never apply this file to a real database.
REVOKE CREATE ON SCHEMA public FROM PUBLIC;
REVOKE TEMP ON DATABASE tablepor_test FROM PUBLIC;

CREATE ROLE tablepor_reader LOGIN PASSWORD 'local-reader-only';
GRANT CONNECT ON DATABASE tablepor_test TO tablepor_reader;
GRANT USAGE ON SCHEMA public TO tablepor_reader;

CREATE TABLE public.customers (
  id bigint PRIMARY KEY,
  name text NOT NULL,
  email text,
  country text NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now()
);
INSERT INTO public.customers VALUES
 (1, 'Mali S.', 'mali@example.test', 'Thailand', '2026-09-10T08:00:00Z'),
 (2, 'Arun K.', 'arun@example.test', 'Thailand', '2026-09-12T10:30:00Z'),
 (3, 'Yuna T.', NULL, 'Japan', '2026-09-14T09:15:00Z');

CREATE TABLE public.orders (
  id bigint PRIMARY KEY,
  customer text NOT NULL,
  status varchar(30) NOT NULL,
  total numeric(20,2) NOT NULL,
  country text NOT NULL,
  created_at timestamptz NOT NULL
);
INSERT INTO public.orders
SELECT i, (ARRAY['Mali S.', 'Arun K.', 'Yuna T.', 'Alex W.'])[(i % 4) + 1],
  (ARRAY['completed', 'processing', 'pending'])[(i % 3) + 1],
  (i * 13.75)::numeric(20,2), (ARRAY['Thailand','Japan','Singapore'])[(i % 3) + 1],
  '2026-09-01T10:00:00Z'::timestamptz + i * interval '1 hour'
FROM generate_series(1, 450) i;
CREATE INDEX orders_status_idx ON public.orders(status);
CREATE INDEX orders_created_at_idx ON public.orders(created_at);
CREATE TABLE public.precision_values (id bigint PRIMARY KEY, amount numeric(30,10), note text);
INSERT INTO public.precision_values VALUES (9007199254740993, 12345678901234567890.1234567890, NULL);

-- Intentionally outside the approved table subset.
CREATE VIEW public.orders_view AS SELECT * FROM public.orders;
CREATE TABLE public.rls_table (id integer);
ALTER TABLE public.rls_table ENABLE ROW LEVEL SECURITY;
CREATE TABLE public.custom_payload (id integer, payload jsonb);
CREATE FUNCTION public.dangerous() RETURNS integer LANGUAGE plpgsql SECURITY DEFINER AS $$
BEGIN
  DELETE FROM public.orders;
  RETURN 1;
END;
$$;
REVOKE ALL ON FUNCTION public.dangerous() FROM PUBLIC;

GRANT SELECT ON ALL TABLES IN SCHEMA public TO tablepor_reader;
ANALYZE;

-- Editing fixture uses column-level UPDATE, never DELETE/TRUNCATE.
CREATE ROLE tablepor_editor LOGIN PASSWORD 'local-editor-only';
GRANT CONNECT ON DATABASE tablepor_test TO tablepor_editor;
GRANT USAGE ON SCHEMA public TO tablepor_editor;
CREATE TABLE public.edit_items (
  id bigint PRIMARY KEY, label text NOT NULL UNIQUE, note text, amount numeric(30,10) NOT NULL,
  label_copy text GENERATED ALWAYS AS (label) STORED
);
INSERT INTO public.edit_items(id,label,note,amount) VALUES
 (1,'First',NULL,12345678901234567890.1234567890),(2,'Second','original',2);
CREATE TABLE public.edit_no_key (label text);
INSERT INTO public.edit_no_key VALUES ('untouched');
CREATE TABLE public.edit_composite (tenant integer, id bigint, label text, PRIMARY KEY(tenant,id));
INSERT INTO public.edit_composite VALUES (1,1,'one'),(2,1,'two');
GRANT SELECT ON ALL TABLES IN SCHEMA public TO tablepor_editor, tablepor_reader;
GRANT UPDATE(label,note,amount) ON public.edit_items TO tablepor_editor;
GRANT UPDATE ON public.edit_no_key, public.edit_composite TO tablepor_editor;

-- Grant UPDATE on the whole table so metadata, not just grants, protects these columns.
CREATE TABLE public.generated_items (
  id integer PRIMARY KEY, sequence bigint GENERATED ALWAYS AS IDENTITY,
  label text, label_copy text GENERATED ALWAYS AS (label) STORED
);
GRANT SELECT ON public.generated_items TO tablepor_reader, tablepor_editor;
GRANT UPDATE ON public.generated_items TO tablepor_editor;

-- PostgreSQL 18 added virtual generated columns, which execute when read.
DO $$ BEGIN
  IF current_setting('server_version_num')::integer >= 180000 THEN
    EXECUTE 'CREATE TABLE public.virtual_items (id integer PRIMARY KEY, computed integer GENERATED ALWAYS AS (id + 1) VIRTUAL)';
    EXECUTE 'GRANT SELECT ON public.virtual_items TO tablepor_reader, tablepor_editor';
  END IF;
END $$;

-- Same table names in different schemas catch wrong-schema reads and saves.
CREATE SCHEMA audit;
CREATE SCHEMA restricted;
GRANT USAGE ON SCHEMA audit TO tablepor_reader, tablepor_editor;
CREATE TABLE audit.edit_items (id bigint PRIMARY KEY, label text NOT NULL);
INSERT INTO audit.edit_items VALUES (1, 'Audit only');
GRANT SELECT ON audit.edit_items TO tablepor_reader, tablepor_editor;
GRANT UPDATE(label) ON audit.edit_items TO tablepor_editor;
CREATE TABLE restricted.edit_items (id bigint PRIMARY KEY, label text);
