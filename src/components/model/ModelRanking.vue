<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import SvgIcon from '@/components/svgIcon/index.vue'
import { getModelRanking } from '@/modules/models/api'
import type { RankedModel } from '@/modules/models/types/contracts'

const { t } = useI18n()

const items = ref<RankedModel[]>([])
const loading = ref(true)

async function load() {
  loading.value = true
  try {
    items.value = await getModelRanking()
  } catch (e) {
    console.warn('load ranking failed', e)
  } finally {
    loading.value = false
  }
}

function scorePercent(s: number) { return Math.round(s * 100) }
function scoreLabel(s: number) { return (s * 10).toFixed(1) }

function approval(it: RankedModel) {
  const n = it.likes + it.dislikes
  return n ? Math.round(it.likes / n * 100) : 0
}
function scoreTip(it: RankedModel) {
  return `综合评分采用贝叶斯平滑（少样本向中位回归）\n好评率 ${approval(it)}% · \u{1F44D}${it.likes} \u{1F44E}${it.dislikes}`
}

function tierClass(s: number) {
  if (s >= 0.85) return 'fill-excellent'
  if (s >= 0.65) return 'fill-good'
  if (s >= 0.40) return 'fill-neutral'
  return 'fill-bad'
}

const MODEL_ICON_MAP: Array<[string, string]> = [
  ['ollama', 'OLLAMA'], ['llamacpp', 'LLAMACPP'], ['llama', 'LLAMACPP'],
  ['claude', 'CLAUDE'], ['gemini', 'GEMINI'], ['deepseek', 'DEEPSEEK'],
  ['qwen', 'Q_WEN'], ['doubao', 'DOUYIN'], ['chatglm', 'ZHIPU'],
  ['zhipu', 'ZHIPU'], ['glm', 'ZHIPU'], ['hunyuan', 'HUNYUAN'],
  ['ernie', 'BAIDU'], ['wenxin', 'BAIDU'], ['baidu', 'BAIDU'],
  ['spark', 'SPARKDESK'], ['openai', 'OPENAI'], ['gpt', 'OPENAI'],
]
function matchIcon(text: string): string | null {
  const lower = text.toLowerCase()
  for (const [k, icon] of MODEL_ICON_MAP) { if (lower.includes(k)) return icon }
  return null
}

onMounted(load)
defineExpose({ load })
</script>

<template>
  <div class="ranking-root">
    <div v-if="loading" class="rk-state">加载中…</div>

    <template v-else-if="items.length">
      <!-- 领奖台 Top 3 -->
      <div class="rk-podium" v-if="items.length >= 3">
        <div
          v-for="item in items.slice(0, 3)"
          :key="item.rank"
          class="rk-podium-card"
          :class="`rk-rank-${item.rank}`"
        >
          <div class="rk-badge" :class="`rk-badge-${item.rank}`">{{ item.rank }}</div>
          <div class="rk-podium-icon">
            <SvgIcon :iconName="matchIcon(item.model_id) || item.provider_code" style="font-size:26px" />
          </div>
          <div class="rk-podium-name" :title="item.model_id">{{ item.model_id }}</div>
          <div class="rk-podium-provider">{{ item.provider_code }}</div>
          <div class="rk-podium-score" :title="scoreTip(item)">
            {{ scoreLabel(item.score) }}<span class="rk-unit">分</span>
          </div>
          <div class="rk-votes">
            <span class="rk-vote up">&#9650; {{ item.likes }}</span>
            <span class="rk-vote down">&#9660; {{ item.dislikes }}</span>
          </div>
        </div>
      </div>

      <!-- 榜单列表（小样本不足3个时也走这里） -->
      <div class="rk-list">
        <div
          v-for="item in (items.length >= 3 ? items.slice(3) : items)"
          :key="item.rank"
          class="rk-row"
        >
          <span class="rk-rank" :class="{ top3: item.rank <= 3 }">
            <span v-if="item.rank <= 3" class="rk-badge sm" :class="`rk-badge-${item.rank}`">{{ item.rank }}</span>
            <template v-else>{{ item.rank }}</template>
          </span>
          <SvgIcon :iconName="matchIcon(item.model_id) || item.provider_code" class="rk-row-icon" style="font-size:20px" />
          <div class="rk-row-info">
            <span class="rk-row-name" :title="item.model_id">{{ item.model_id }}</span>
            <span class="rk-row-provider">{{ item.provider_code }}</span>
          </div>
          <div class="rk-bar-track">
            <div class="rk-bar-fill" :class="tierClass(item.score)" :style="{ width: scorePercent(item.score) + '%' }" />
          </div>
          <span class="rk-score" :title="scoreTip(item)">{{ scoreLabel(item.score) }}</span>
          <span class="rk-row-votes">
            <span class="up">&#9650;{{ item.likes }}</span><span class="down">&#9660;{{ item.dislikes }}</span>
          </span>
        </div>
      </div>
    </template>

    <div v-else class="rk-state rk-empty">
      <SvgIcon iconName="排行榜" style="font-size:30px;opacity:.4" />
      <p>{{ t('modelManage.noRatingData') }}</p>
    </div>
  </div>
</template>

<style lang="scss" scoped>
$gold: #f59e0b; $silver: #94a3b8; $bronze: #d97706;

.ranking-root { width: 100%; padding: 4px 2px 12px; }

.rk-state {
  padding: 64px 0; text-align: center;
  color: var(--text-secondary); font-size: 14px;
}
.rk-empty { display: flex; flex-direction: column; align-items: center; gap: 12px; }

/* ════════ 领奖台 ════════ */
.rk-podium {
  display: flex; justify-content: center; align-items: flex-end;
  gap: 16px; padding: 26px 0 34px;
}
.rk-podium-card {
  position: relative;
  display: flex; flex-direction: column; align-items: center; gap: 5px;
  width: 168px; padding: 26px 18px 16px;
  border-radius: 16px;
  background: var(--bg-primary);
  border: 1px solid var(--border-color);
  box-shadow: 0 1px 2px rgba(16, 24, 40, 0.04);
  transition: transform .18s ease, box-shadow .18s ease;
  &:hover { transform: translateY(-4px); box-shadow: 0 10px 24px rgba(16, 24, 40, 0.10); }
}
/* 冠军居中、放大、金色光环 */
.rk-rank-1 {
  order: 2; transform: scale(1.08);
  border-color: rgba(245, 158, 11, .45);
  background: linear-gradient(180deg, rgba(245,158,11,.10) 0%, var(--bg-primary) 62%);
  box-shadow: 0 10px 30px rgba(245, 158, 11, 0.18);
  &:hover { transform: scale(1.08) translateY(-4px); }
}
.rk-rank-2 {
  order: 1;
  border-color: rgba(148, 163, 184, .45);
  background: linear-gradient(180deg, rgba(148,163,184,.10) 0%, var(--bg-primary) 62%);
}
.rk-rank-3 {
  order: 3;
  border-color: rgba(217, 119, 6, .42);
  background: linear-gradient(180deg, rgba(217,119,6,.10) 0%, var(--bg-primary) 62%);
}

/* 圆形名次徽章（替代 emoji） */
.rk-badge {
  position: absolute; top: -14px; left: 50%; transform: translateX(-50%);
  width: 28px; height: 28px; border-radius: 50%;
  display: flex; align-items: center; justify-content: center;
  font-size: 13px; font-weight: 800; color: #fff;
  box-shadow: 0 2px 6px rgba(0,0,0,.18);
  &.sm { position: static; transform: none; width: 22px; height: 22px; font-size: 12px; box-shadow: none; }
}
.rk-badge-1 { background: linear-gradient(135deg, #fbbf24, $gold); }
.rk-badge-2 { background: linear-gradient(135deg, #cbd5e1, $silver); }
.rk-badge-3 { background: linear-gradient(135deg, #fbbf77, $bronze); }

.rk-podium-icon {
  width: 46px; height: 46px; border-radius: 13px; margin-top: 6px;
  display: flex; align-items: center; justify-content: center;
  background: var(--bg-hover);
}
.rk-podium-name {
  max-width: 144px; font-size: 14px; font-weight: 700; color: var(--text-primary);
  overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}
.rk-podium-provider { font-size: 11px; color: var(--text-secondary); opacity: .8; }
.rk-podium-score {
  margin-top: 2px; font-size: 26px; font-weight: 800; color: var(--text-primary);
  cursor: default;
  .rk-unit { font-size: 12px; font-weight: 500; color: var(--text-secondary); margin-left: 2px; }
}
.rk-votes { display: flex; gap: 8px; margin-top: 2px; }
.rk-vote {
  font-size: 11px; padding: 1px 8px; border-radius: 999px; font-weight: 600;
  &.up   { color: #16a34a; background: rgba(34,197,94,.10); }
  &.down { color: #dc2626; background: rgba(239,68,68,.10); }
}

/* ════════ 榜单列表 ════════ */
.rk-list { display: flex; flex-direction: column; gap: 6px; padding: 0 4px; }
.rk-row {
  display: flex; align-items: center; gap: 12px;
  padding: 10px 14px; border-radius: 12px;
  background: var(--bg-primary);
  border: 1px solid transparent;
  transition: border-color .15s ease, background .15s ease, transform .15s ease;
  &:hover { border-color: var(--border-color); background: var(--bg-hover); transform: translateX(2px); }
}
.rk-rank {
  width: 30px; flex-shrink: 0; text-align: center;
  font-size: 14px; font-weight: 700; color: var(--text-secondary);
  display: flex; align-items: center; justify-content: center;
}
.rk-row-icon { flex-shrink: 0; }
.rk-row-info { width: 150px; flex-shrink: 0; display: flex; flex-direction: column; gap: 1px; min-width: 0; }
.rk-row-name { font-size: 13px; font-weight: 600; color: var(--text-primary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.rk-row-provider { font-size: 11px; color: var(--text-secondary); opacity: .8; }

.rk-bar-track {
  flex: 1; height: 7px; border-radius: 999px; overflow: hidden;
  background: var(--bg-hover);
}
.rk-bar-fill {
  height: 100%; border-radius: 999px; min-width: 5px;
  transition: width .6s cubic-bezier(.22,1,.36,1);
}
.fill-excellent { background: linear-gradient(90deg, #34d399, #16a34a); }
.fill-good      { background: linear-gradient(90deg, #5aa9ff, var(--color-primary)); }
.fill-neutral   { background: linear-gradient(90deg, #fcd34d, #f59e0b); }
.fill-bad       { background: linear-gradient(90deg, #f87171, #dc2626); }

.rk-score { width: 38px; flex-shrink: 0; text-align: right; font-size: 14px; font-weight: 700; color: var(--text-primary); cursor: default; }
.rk-row-votes {
  width: 86px; flex-shrink: 0; display: flex; justify-content: flex-end; gap: 8px; font-size: 11px; font-weight: 600;
  .up { color: #16a34a; } .down { color: #dc2626; }
}

@media (max-width: 720px) {
  .rk-row-info { width: 110px; }
  .rk-podium-card { width: 132px; }
}
</style>
