declare module '@/api/newApi' {
  interface LogApi {
    get(data?: Record<string, any>): Promise<any>
    getStat(data?: Record<string, any>): Promise<any>
    delete(id: number | string): Promise<any>
  }

  interface DrawingLogApi {
    get(data?: Record<string, any>): Promise<any>
  }

  interface TaskLogApi {
    get(data?: Record<string, any>): Promise<any>
  }

  interface UserApi {
    self(data?: Record<string, any>): Promise<any>
    aff(data?: Record<string, any>): Promise<any>
    info(): Promise<any>
    amount(data?: Record<string, any>): Promise<any>
    topup(data?: Record<string, any>): Promise<any>
  }

  interface SysApi {
    status(data?: Record<string, any>): Promise<any>
  }

  interface DataApi {
    self(data?: Record<string, any>): Promise<any>
  }

  interface ApiTokenApi {
    get(data?: Record<string, any>): Promise<any>
    search(data?: Record<string, any>): Promise<any>
    save(data?: Record<string, any>): Promise<any>
    remove(id: number | string): Promise<any>
    batchRemove(data?: Record<string, any>): Promise<any>
    update(data?: Record<string, any>): Promise<any>
    updateStatus(data?: Record<string, any>): Promise<any>
    getKey(id: number | string): Promise<any>
  }

  interface NewApi {
    apiToken: ApiTokenApi
    useLogApi: LogApi
    drawingLogApi: DrawingLogApi
    taskLogApi: TaskLogApi
    userApi: UserApi
    sysApi: SysApi
    dataApi: DataApi
  }

  const api: NewApi
  export default api
}
