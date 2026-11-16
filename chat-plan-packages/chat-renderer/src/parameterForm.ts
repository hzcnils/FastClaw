export interface ChatParameterField {
  key: string
  label: string
  type?: 'text' | 'password' | 'url'
  placeholder?: string
  required?: boolean
}

export type ChatParameterValues = Record<string, string>
