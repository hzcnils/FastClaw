<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { Close } from '@element-plus/icons-vue'

defineProps<{
  quotedText: string
  selectedSkill: { type: string; labelEn: string } | null
}>()
const emit = defineEmits<{ clearQuote: []; clearSkill: [] }>()
const { t } = useI18n()
</script>

<template>
          <div v-if="quotedText" class="quote-ref-bar">
            <span class="quote-ref-text" :title="quotedText">{{ quotedText }}</span>
            <button class="quote-ref-close" type="button" :title="t('common.cancel')" @click="emit('clearQuote')"><el-icon :size="14"><Close /></el-icon></button>
          </div>
          <div v-if="selectedSkill" class="selected-skill-chip">
            <span class="chip-badge" :class="selectedSkill.type">{{ selectedSkill.type === 'mcp' ? 'MCP' : 'Skill' }}</span>
            <span class="chip-name">{{ selectedSkill.labelEn }}</span>
            <button class="chip-close" @click="emit('clearSkill')">×</button>
          </div>
</template>

<style scoped lang="scss">
@use './composerContextHints';
</style>
