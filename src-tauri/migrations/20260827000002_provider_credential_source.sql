-- API keys are provider-level credentials. New writes no longer duplicate them
-- in models.extra_config. Remove only historical values that are provably null
-- or identical to the canonical provider value. Conflicting/local legacy values
-- remain inert for compatibility and are never used as a credential source.
UPDATE models
SET extra_config = json_remove(extra_config, '$.apiKey')
WHERE json_valid(extra_config)
  AND json_type(extra_config, '$.apiKey') IS NOT NULL
  AND (
    json_extract(extra_config, '$.apiKey') IS NULL
    OR EXISTS (
      SELECT 1
      FROM model_providers p
      WHERE p.provider_code = models.provider_code
        AND COALESCE(p.api_key_ref, '') <> ''
        AND p.api_key_ref = json_extract(models.extra_config, '$.apiKey')
    )
  );
