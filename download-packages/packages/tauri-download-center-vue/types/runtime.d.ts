import { type ComputedRef, type InjectionKey, type Ref } from 'vue';
import { type ApiError, type DownloadClientOptions, type DownloadTask, type StartDownloadInput } from 'tauri-download-center-core';
import type { DownloadCenterUiConfig, DownloadCenterUiConfigInput } from './uiConfig';
import { createDownloadCenterText } from './uiConfig';
export type DownloadEventTransport = <T>(eventName: string, handler: (payload: T) => void) => Promise<() => void>;
export interface DownloadRuntimeOptions {
    config: DownloadCenterUiConfig;
    client?: DownloadClientOptions;
    eventName?: string;
    listen?: DownloadEventTransport;
    onTaskUpdated?: (task: DownloadTask) => void | Promise<void>;
    onTaskCompleted?: (task: DownloadTask) => void | Promise<void>;
    onTaskFailed?: (task: DownloadTask) => void | Promise<void>;
}
export interface DownloadRuntime {
    config: DownloadCenterUiConfig;
    text: ReturnType<typeof createDownloadCenterText>;
    configureUi(input: DownloadCenterUiConfigInput): void;
    tasks: Readonly<Ref<readonly DownloadTask[]>>;
    panelVisible: Ref<boolean>;
    activeCount: ComputedRef<number>;
    loading: Readonly<Ref<boolean>>;
    error: Readonly<Ref<ApiError | null>>;
    initialize(): Promise<void>;
    start(input: StartDownloadInput): Promise<DownloadTask>;
    pause(id: string): Promise<DownloadTask>;
    resume(id: string): Promise<DownloadTask>;
    cancel(id: string): Promise<DownloadTask>;
    retry(id: string): Promise<DownloadTask>;
    remove(id: string): Promise<void>;
    openPanel(): void;
    closePanel(): void;
    dispose(): void;
}
export declare const downloadRuntimeKey: InjectionKey<DownloadRuntime>;
export declare function createDownloadRuntime(options: DownloadRuntimeOptions): DownloadRuntime;
export declare function useDownloadRuntime(): DownloadRuntime;
