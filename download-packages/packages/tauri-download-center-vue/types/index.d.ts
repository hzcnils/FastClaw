import type { App } from 'vue';
import DownloadCenterButton from './DownloadButton.vue';
import DownloadCenterPanel from './DownloadCenter.vue';
import DownloadTaskItem from './DownloadTaskItem.vue';
import { type DownloadEventTransport, type DownloadRuntime, type DownloadRuntimeOptions } from './runtime';
import type { DownloadClientOptions } from 'tauri-download-center-core';
import { type DownloadCenterUiConfigInput } from './uiConfig';
export * from 'tauri-download-center-core';
export * from './uiConfig';
export * from './runtime';
export { DownloadCenterButton, DownloadCenterPanel, DownloadTaskItem };
export interface DownloadCenterVuePluginOptions extends DownloadCenterUiConfigInput {
    i18n?: {
        global?: {
            mergeLocaleMessage(locale: string, message: Record<string, unknown>): void;
        };
        mergeLocaleMessage?(locale: string, message: Record<string, unknown>): void;
    };
    client?: DownloadClientOptions;
    eventName?: string;
    listen?: DownloadEventTransport;
    onTaskUpdated?: DownloadRuntimeOptions['onTaskUpdated'];
    onTaskCompleted?: DownloadRuntimeOptions['onTaskCompleted'];
    onTaskFailed?: DownloadRuntimeOptions['onTaskFailed'];
    runtimeInstance?: DownloadRuntime;
}
export declare const DownloadCenterVuePlugin: {
    install(app: App, options?: DownloadCenterVuePluginOptions): void;
};
export default DownloadCenterVuePlugin;
