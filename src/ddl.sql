-- Strip top-level DML from an event trigger's current_query(). Logical replication
-- already carries those rows. Preserve quoted bodies and nested comments while
-- splitting statements; unsupported procedural batches fail closed for repair.
CREATE OR REPLACE FUNCTION @SCHEMA@.ddl_only(q text) RETURNS text LANGUAGE plpgsql AS $lexer$
DECLARE
  i integer := 1; n integer := length(q); c text; nxt text;
  quote text := ''; dollar text := ''; tag text; escaped boolean := false;
  depth integer := 0; line_comment boolean := false;
  statement text := ''; statements text[] := ARRAY[]::text[];
  item text; keyword text; result text := '';
BEGIN
  IF n > 1048576 THEN RAISE EXCEPTION 'DDL batch exceeds 1 MiB; send individual migration statements'; END IF;
  WHILE i <= n LOOP
    c := substr(q,i,1); nxt := substr(q,i,2);
    IF line_comment THEN
      IF c = chr(10) THEN line_comment := false; statement := statement || ' '; END IF;
    ELSIF depth > 0 THEN
      IF nxt = '/*' THEN depth := depth+1; i := i+1;
      ELSIF nxt = '*/' THEN depth := depth-1; i := i+1; END IF;
    ELSIF dollar <> '' THEN
      IF substr(q,i,length(dollar)) = dollar THEN
        statement := statement || dollar; i := i+length(dollar)-1; dollar := '';
      ELSE statement := statement || c; END IF;
    ELSIF quote <> '' THEN
      statement := statement || c;
      IF c = chr(92) AND escaped THEN
        i := i+1; statement := statement || substr(q,i,1);
      ELSIF c = quote THEN
        IF substr(q,i+1,1) = quote THEN i := i+1; statement := statement || quote;
        ELSE quote := ''; END IF;
      END IF;
    ELSIF nxt = '--' THEN line_comment := true; i := i+1;
    ELSIF nxt = '/*' THEN depth := 1; i := i+1; statement := statement || ' ';
    ELSIF c IN (chr(39),chr(34)) THEN
      quote := c;
      escaped := c = chr(39) AND (lower(substr(q,greatest(i-1,1),1)) = 'e' OR current_setting('standard_conforming_strings') = 'off');
      statement := statement || c;
    ELSIF c = '$' THEN
      tag := substring(substr(q,i) from '^\$([a-zA-Z_][a-zA-Z_0-9]*)?\$');
      -- substring with parentheses returns the capture, so obtain the whole tag.
      IF substr(q,i,2) = '$$' THEN dollar := '$$';
      ELSIF tag IS NOT NULL THEN dollar := '$' || tag || '$'; END IF;
      IF dollar <> '' THEN statement := statement || dollar; i := i+length(dollar)-1;
      ELSE statement := statement || c; END IF;
    ELSIF c = ';' THEN statements := array_append(statements,statement); statement := '';
    ELSE statement := statement || c;
    END IF;
    i := i+1;
  END LOOP;
  IF quote <> '' OR dollar <> '' OR depth > 0 THEN RAISE EXCEPTION 'unclosed quote or comment in DDL batch'; END IF;
  statements := array_append(statements,statement);
  FOREACH item IN ARRAY statements LOOP
    item := btrim(item); IF item = '' THEN CONTINUE; END IF;
    keyword := upper(substring(item from '^[a-zA-Z]+'));
    IF keyword IN ('ALTER','CREATE','DROP','COMMENT','GRANT','REVOKE','SECURITY','REINDEX') THEN
      IF item ~* '^CREATE\s+((TEMP|TEMPORARY|UNLOGGED)\s+)?TABLE\s+.*\s+AS\s+' THEN
        RAISE EXCEPTION 'CREATE TABLE AS requires explicit schema reconciliation';
      END IF;
      item := regexp_replace(item, '^((CREATE(\s+UNIQUE)?|DROP|REINDEX)\s+INDEX)\s+CONCURRENTLY\s+', '\1 ', 'i');
      result := result || item || ';';
    ELSIF keyword IN ('INSERT','UPDATE','DELETE','MERGE','SELECT','WITH','SET','RESET','BEGIN','COMMIT','ROLLBACK','START','END','SAVEPOINT','RELEASE') THEN
      CONTINUE;
    ELSE RAISE EXCEPTION 'unsupported DDL batch statement: %; send individual migration statements', keyword;
    END IF;
  END LOOP;
  IF result = '' THEN RAISE EXCEPTION 'no directly replayable DDL in batch; reconcile the schema explicitly'; END IF;
  RETURN result;
END $lexer$;
