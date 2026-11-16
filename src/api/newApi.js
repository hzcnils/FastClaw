import http from "@/utils/request";

const baseURL = `/newApi/api`
export default {
  apiToken: { // API令牌
    get: async function (data) {
      return http.get(`${baseURL}/token/`, data);
    },
    search: async function (data) {
      return http.get(`${baseURL}/token/search`, data);
    },
    save: async function (data) {
      return http.post(`${baseURL}/token/`, data);
    },
    remove: async function (id) {
      return http.delete(`${baseURL}/token/${id}`);
    },
    batchRemove: async function (data) {
      return http.post(`${baseURL}/token/batch`, data);
    },
    update: async function (data) {
      return http.put(`${baseURL}/token/`, data);
    },
    updateStatus: async function (data) { // 更新状态
      return http.put(`${baseURL}/token/?status_only=true`, data);
    },
    getKey: async function (id) {
      return http.post(`${baseURL}/token/${id}/key`);
    },
  },
  useLogApi: { // 使用日志接口
    get: async function (data) {
      return http.get(`${baseURL}/log/self`, { params: data });
    },
    getStat: async function (data) {
      return http.get(`${baseURL}/log/self/stat`, { params: data });
    },
    delete: async function (id) {
      return http.delete(`${baseURL}/log/self/${id}`);
    },
  },
  drawingLogApi: { // 绘图日志接口
    get: async function (data) {
      return http.get(`${baseURL}/mj/`, data);
    }
  },
  taskLogApi: { // 任务接口
    get: async function (data) {
      return http.get(`${baseURL}/task/`, data);
    },
  },
  userApi: { // 用户接口
    self: async function (data) {
      return http.get(`${baseURL}/user/self`, data);
    },
    aff: async function (data) {
      return http.get(`${baseURL}/user/aff`, data);
    },
    info: async function () {
      return http.get(`${baseURL}/user/topup/info`);
    },
    amount: async function (data) {
      return http.post(`${baseURL}/user/amount`, data);
    },
    topup: async function (data) {
      return http.post(`${baseURL}/user/topup`, data);
    },
  },
  sysApi: { // 系统接口
    status: async function (data) {
      return http.get(`${baseURL}/status`, data);
    },
  },
  dataApi: { // 数据接口
    self: async function (data) {
      return http.get(`${baseURL}/data/self`, { params: data });
    },
  }

}