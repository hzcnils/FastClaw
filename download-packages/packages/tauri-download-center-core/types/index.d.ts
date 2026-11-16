export declare const DOWNLOAD_CENTER_IPC_PROTOCOL_VERSION = 1;
export interface ApiError {
    code: string;
    message: string;
    details?: unknown;
}
export interface ApiResult<T> {
    success: boolean;
    data: T | null;
    error: ApiError | null;
}
export type InvokeTransport = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;
export interface DownloadClientOptions {
    invoke?: InvokeTransport;
    command?: string;
}
export type DownloadStatus = 'pending' | 'downloading' | 'pausing' | 'paused' | 'verifying' | 'installing' | 'success' | 'failed' | 'cancelled';
export type DownloadInstruction = {
    type: 'download';
    url: string;
    tempPath: string;
    resume?: boolean;
} | {
    type: 'verifySha256';
    sha256: string;
} | {
    type: 'extractZip';
    stagingPath: string;
    requiredFileSuffix?: string;
} | {
    type: 'backupTarget';
    targetPath: string;
    backupPath?: string;
} | {
    type: 'replaceTarget';
    source: 'temp' | 'staging';
    targetPath: string;
} | {
    type: 'overlayTarget';
    source: 'temp' | 'staging';
    targetPath: string;
} | {
    type: 'cleanup';
    targets: Array<'temp' | 'staging' | 'backup'>;
};
export interface DownloadExecutionPlan {
    instructions: DownloadInstruction[];
}
export interface DownloadTask {
    id: string;
    resourceType: string;
    resourceId: string;
    name: string;
    status: DownloadStatus;
    phase: string;
    progress: number;
    downloadedBytes: number;
    totalBytes?: number;
    messageKey?: string;
    messageParams?: Record<string, string | number>;
    error?: ApiError;
    silent?: boolean;
    payload?: Record<string, unknown>;
    createdAt: string;
    updatedAt: string;
}
export interface StartDownloadInput {
    resourceType: string;
    resourceId: string;
    name: string;
    payload: Record<string, unknown>;
    silent?: boolean;
}
export declare class DownloadApiError extends Error {
    readonly apiError: ApiError;
    constructor(apiError: ApiError);
}
export declare function createDownloadClient(options?: DownloadClientOptions): <T>(action: string, payload?: unknown) => Promise<T>;
export interface DownloadStateSnapshot<T = unknown> {
    tasks: T[];
    loading: boolean;
    error: ApiError | null;
}
export declare class DownloadState<T = unknown> {
    private value;
    private readonly listeners;
    get snapshot(): Readonly<DownloadStateSnapshot<T>>;
    subscribe(listener: (value: DownloadStateSnapshot<T>) => void): () => void;
    patch(patch: Partial<DownloadStateSnapshot<T>>): void;
}
