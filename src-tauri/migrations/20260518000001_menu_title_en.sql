-- Add title_en column for bilingual menu support
ALTER TABLE custom_menus ADD COLUMN title_en TEXT DEFAULT NULL;
