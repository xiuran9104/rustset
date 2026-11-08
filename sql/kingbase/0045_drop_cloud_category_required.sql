-- cloud_category was a free-text catch-all from before structured bindings:
-- cloud resources now derive region/zone from their platform, physical
-- resources carry machine_room_id and deployment_type. The form no longer
-- collects it; the column stays nullable for historical rows.
ALTER TABLE public.infra_cloud_resource
    ALTER COLUMN cloud_category DROP NOT NULL;
ALTER TABLE public.infra_physical_resource
    ALTER COLUMN cloud_category DROP NOT NULL;
