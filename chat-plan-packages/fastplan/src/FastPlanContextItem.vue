<script setup lang="ts">
type ContextOption = { id: string; label: string; disabled?: boolean }
type ContextItem = { id: string; label: string; value?: string; icon?: string; badge?: string | number; active?: boolean; disabled?: boolean; options?: ContextOption[]; selectedOptionId?: string }

const props = defineProps<{ item: ContextItem; select: (item: ContextItem, event: MouseEvent | KeyboardEvent) => void }>()
const emit = defineEmits<{ (event: 'option-select', value: { item: ContextItem; option: ContextOption; nativeEvent: Event }): void }>()

function selectOption(event: Event) {
  const option = props.item.options?.find(entry => entry.id === (event.target as HTMLSelectElement).value)
  if (option && !option.disabled) emit('option-select', { item: props.item, option, nativeEvent: event })
}
</script>

<template>
  <label v-if="item.options?.length" class="jcr-context-item" :class="{ 'is-active': item.active }">
    <span v-if="item.icon" class="jcr-context-item__icon" aria-hidden="true">{{ item.icon }}</span>
    <span>{{ item.label }}</span>
    <select :value="item.selectedOptionId" :disabled="item.disabled" @change="selectOption">
      <option v-for="option in item.options" :key="option.id" :value="option.id" :disabled="option.disabled">{{ option.label }}</option>
    </select>
  </label>
  <button v-else type="button" class="jcr-context-item" :class="{ 'is-active': item.active }" :disabled="item.disabled" :aria-pressed="item.active || undefined" @click="select(item, $event)">
    <span v-if="item.icon" class="jcr-context-item__icon" aria-hidden="true">{{ item.icon }}</span>
    <span>{{ item.label }}</span><strong v-if="item.value">{{ item.value }}</strong><small v-if="item.badge !== undefined">{{ item.badge }}</small>
  </button>
</template>
