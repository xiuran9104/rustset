ALTER TABLE public.cmdb_attribute
    ADD COLUMN IF NOT EXISTS color varchar(16);

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'cmdb_attribute_color_check'
    ) THEN
        ALTER TABLE public.cmdb_attribute
            ADD CONSTRAINT cmdb_attribute_color_check
            CHECK (color IS NULL OR color ~ '^#[0-9A-Fa-f]{6}([0-9A-Fa-f]{2})?$');
    END IF;
END $$;
