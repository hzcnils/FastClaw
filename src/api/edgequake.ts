import { http, handleResponse } from './knowledge';

export interface EdgeQuakeConfig {
  base_url: string;
  api_key?: string;
  tenant_id?: string;
}

export interface EdgeQuakeHealth {
  status: string;
  version?: string;
  message?: string;
}

export interface EdgeQuakeSource {
  document_id?: string;
  chunk_text?: string;
  score?: number;
}

export interface EdgeQuakeQueryRequest {
  query: string;
  top_k?: number;
  filters?: any;
}

export interface EdgeQuakeQueryResponse {
  answer?: string;
  sources?: EdgeQuakeSource[];
  latency_ms?: number;
}

export const edgequakeApi = {
  health: () => handleResponse<EdgeQuakeHealth>(http.get('/edgequake/health')),

  query: (data: EdgeQuakeQueryRequest) =>
    handleResponse<EdgeQuakeQueryResponse>(http.post('/edgequake/query', data)),
};
