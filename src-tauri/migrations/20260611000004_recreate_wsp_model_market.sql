-- 重建 wsp_model_market：去掉 NOT NULL 约束和 skillName 唯一约束
DROP TABLE IF EXISTS wsp_model_market;

CREATE TABLE IF NOT EXISTS wsp_model_market (
    id                  INTEGER NOT NULL,
    exeName             TEXT NOT NULL,
    exeOriginName       TEXT,
    exePath             TEXT,
    exeRelativePath     TEXT,
    exeSize             TEXT,
    exeStoreType        TEXT,
    lastRunTime         TEXT,
    categoryId          INTEGER,
    subCategoryId       INTEGER,
    skillName           TEXT,
    skillDescription    TEXT,
    skillInstruction    TEXT,
    skillLicense        TEXT,
    skillCompatibility  TEXT,
    skillVersion        TEXT,
    skillUrl            TEXT,
    skillWellKnown      TEXT,
    skillEntryPoint     TEXT,
    startupArgs         TEXT,
    startupApiUrl       TEXT,
    startupApiMethod    TEXT,
    startupApiParams    TEXT,
    tag                 TEXT,
    remark              TEXT,
    createUser          TEXT,
    createDept          TEXT,
    createTime          TEXT,
    updateUser          TEXT,
    updateTime          TEXT,
    status              INTEGER,
    isDeleted           INTEGER,
    tenantId            TEXT NOT NULL DEFAULT '000000',
    PRIMARY KEY (id),
    UNIQUE (exeName)
);

CREATE INDEX IF NOT EXISTS idx_wsp_market_category ON wsp_model_market (categoryId);
CREATE INDEX IF NOT EXISTS idx_wsp_market_tenant_status ON wsp_model_market (tenantId, status, isDeleted);
CREATE INDEX IF NOT EXISTS idx_wsp_market_last_run ON wsp_model_market (lastRunTime);
