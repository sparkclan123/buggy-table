CREATE ROLE vpn_reader LOGIN PASSWORD 'disposable-vpn-test' NOSUPERUSER NOCREATEDB NOCREATEROLE;
REVOKE ALL ON DATABASE fixture FROM PUBLIC;
CREATE TABLE public.items (id integer PRIMARY KEY, label text NOT NULL);
INSERT INTO public.items VALUES (1, 'VPN fixture');
GRANT CONNECT ON DATABASE fixture TO vpn_reader;
GRANT USAGE ON SCHEMA public TO vpn_reader;
GRANT SELECT ON public.items TO vpn_reader;
