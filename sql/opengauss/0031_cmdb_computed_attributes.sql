-- Computed attributes store a constrained numeric formula. Empty expression
-- means the attribute remains an ordinary user supplied field.
DO $$ BEGIN ALTER TABLE public.cmdb_attribute ADD COLUMN expression text; EXCEPTION WHEN duplicate_column THEN NULL; END $$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'cmdb_attribute_expression_type_check'
    ) THEN
        ALTER TABLE public.cmdb_attribute
            ADD CONSTRAINT cmdb_attribute_expression_type_check
            CHECK (expression IS NULL OR attr_type IN ('number', 'float'));
    END IF;
END $$;
