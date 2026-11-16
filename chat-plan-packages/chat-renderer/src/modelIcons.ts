export type BrandedModelIconKey =
  | 'OPENAI' | 'CLAUDE' | 'GEMINI' | 'DEEPSEEK' | 'Q_WEN' | 'DOUYIN'
  | 'ZHIPU' | 'HUNYUAN' | 'BAIDU' | 'SPARKDESK' | 'CLOUDFLARE'
  | 'PERPLEXITY' | 'COHERE' | 'JINA' | 'GROK' | 'MOONSHOT'
  | 'OLLAMA' | 'LLAMACPP' | 'NEWAPI' | 'JIMENG'

export type ModelIconKey = BrandedModelIconKey | 'CI' | 'MODEL'

const modelKeywords: Array<[string, ModelIconKey]> = [
  ['ollama', 'OLLAMA'], ['llamacpp', 'LLAMACPP'], ['llama', 'LLAMACPP'],
  ['newapi', 'NEWAPI'], ['jimeng', 'JIMENG'],
  ['claude', 'CLAUDE'], ['anthropic', 'CLAUDE'], ['jmiscode', 'CLAUDE'],
  ['openai', 'OPENAI'], ['gpt', 'OPENAI'], ['azure', 'OPENAI'], ['codex', 'OPENAI'],
  ['gemini', 'GEMINI'], ['google', 'GEMINI'], ['deepseek', 'DEEPSEEK'],
  ['qwen', 'Q_WEN'], ['qianwen', 'Q_WEN'], ['dashscope', 'Q_WEN'],
  ['doubao', 'DOUYIN'], ['bytedance', 'DOUYIN'],
  ['chatglm', 'ZHIPU'], ['zhipu', 'ZHIPU'], ['glm', 'ZHIPU'], ['zai', 'ZHIPU'],
  ['hunyuan', 'HUNYUAN'], ['ernie', 'BAIDU'], ['wenxin', 'BAIDU'], ['baidu', 'BAIDU'],
  ['spark', 'SPARKDESK'], ['grok', 'GROK'], ['xai', 'GROK'],
  ['moonshot', 'MOONSHOT'], ['kimi', 'MOONSHOT'], ['perplexity', 'PERPLEXITY'],
  ['cohere', 'COHERE'], ['mistral', 'CLAUDE'], ['siliconflow', 'CI'], ['huizhi', 'CI'],
  ['jina', 'JINA'], ['cloudflare', 'CLOUDFLARE'], ['nebius', 'CLAUDE'],
  ['cerebras', 'CLAUDE'], ['groq', 'CLAUDE'], ['deepinfra', 'CLAUDE'],
  ['togetherai', 'CLAUDE'], ['together', 'CLAUDE'], ['sambanova', 'CLAUDE'],
  ['fireworks', 'CLAUDE'], ['novita', 'CLAUDE'], ['openrouter', 'CLAUDE'],
  ['stepfun', 'CLAUDE'], ['upstage', 'CLAUDE'],
]

export const MODEL_ICON_LABELS: Record<ModelIconKey, string> = {
  OPENAI: 'O', CLAUDE: 'C', GEMINI: 'G', DEEPSEEK: 'D', Q_WEN: 'Q', DOUYIN: 'DB',
  ZHIPU: 'Z', HUNYUAN: 'HY', BAIDU: 'B', SPARKDESK: 'XF', CLOUDFLARE: 'CF',
  PERPLEXITY: 'P', COHERE: 'CO', JINA: 'J', GROK: 'X', MOONSHOT: 'K',
  OLLAMA: 'OL', LLAMACPP: 'L', NEWAPI: 'N', JIMENG: 'JM', CI: 'CI', MODEL: 'AI',
}

export function matchModelIcon(text?: string): ModelIconKey {
  const normalized = (text || '').toLowerCase().replace(/[\s_\-/]+/g, '')
  return modelKeywords.find(([keyword]) => normalized.includes(keyword))?.[1] || 'MODEL'
}
