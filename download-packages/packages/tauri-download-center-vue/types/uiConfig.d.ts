export interface DownloadCenterUiMessages {
    title: string;
    empty: string;
    active: string;
    pause: string;
    resume: string;
    cancel: string;
    retry: string;
    remove: string;
    close: string;
    statusPending: string;
    statusDownloading: string;
    statusPausing: string;
    statusPaused: string;
    statusVerifying: string;
    statusInstalling: string;
    statusSuccess: string;
    statusFailed: string;
    statusCancelled: string;
    [messageKey: string]: string;
}
export interface DownloadCenterUiConfig {
    locale: string;
    title: string;
    emptyText: string;
    messages: Record<string, {
        downloadCenter: DownloadCenterUiMessages;
    }>;
}
export type DownloadCenterUiConfigInput = Partial<Omit<DownloadCenterUiConfig, 'messages'>> & {
    messages?: Record<string, {
        downloadCenter?: Partial<DownloadCenterUiMessages>;
    }>;
};
export declare const defaultDownloadCenterUiConfig: DownloadCenterUiConfig;
export declare function resolveDownloadCenterUiConfig(input?: DownloadCenterUiConfigInput, base?: DownloadCenterUiConfig): DownloadCenterUiConfig;
export declare function createDownloadCenterText(config: DownloadCenterUiConfig): (key: keyof DownloadCenterUiMessages | string, params?: Record<string, string | number>) => string;
