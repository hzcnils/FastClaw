import { ElMessage } from 'element-plus'

export function useToast() {
  return {
    success: (msg: string) => ElMessage.success(msg),
    error:   (msg: string) => ElMessage.error(msg),
    info:    (msg: string) => ElMessage.info(msg),
  }
}
