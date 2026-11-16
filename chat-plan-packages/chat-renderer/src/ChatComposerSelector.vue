<script setup lang="ts">
import { computed, ref } from 'vue'
import type {
  ChatComposerSelector,
  ChatComposerSelectorChangeEvent,
  ChatComposerSelectorOption,
} from './types'

const props = defineProps<{
  selector: ChatComposerSelector
}>()

const emit = defineEmits<{
  (event: 'change', value: ChatComposerSelectorChangeEvent): void
}>()

const root = ref<HTMLDetailsElement | null>(null)
const query = ref('')

const values = computed(() => {
  const value = props.selector.value
  if (Array.isArray(value)) return value.map(String)
  return value == null || value === '' ? [] : [String(value)]
})

const selected = computed(() => props.selector.options.filter(option => values.value.includes(String(option.value))))
const filteredOptions = computed(() => {
  const needle = query.value.trim().toLocaleLowerCase()
  if (!needle) return props.selector.options
  return props.selector.options.filter(option =>
    [option.label, option.description, option.group]
      .filter(Boolean)
      .some(value => String(value).toLocaleLowerCase().includes(needle)),
  )
})
const groups = computed(() => {
  const result: Array<{ label: string; options: ChatComposerSelectorOption[] }> = []
  for (const option of filteredOptions.value) {
    const label = option.group || ''
    let group = result.find(item => item.label === label)
    if (!group) {
      group = { label, options: [] }
      result.push(group)
    }
    group.options.push(option)
  }
  return result
})
const summary = computed(() => {
  if (!selected.value.length) return props.selector.placeholder || props.selector.label
  const max = Math.max(1, props.selector.maxVisibleValues || 2)
  const labels = selected.value.slice(0, max).map(option => option.label)
  const remaining = selected.value.length - labels.length
  return remaining > 0 ? `${labels.join('、')} +${remaining}` : labels.join('、')
})

function publish(next: string[], nativeEvent: MouseEvent | KeyboardEvent) {
  const value = props.selector.multiple ? next : (next[0] || '')
  emit('change', {
    selector: props.selector,
    value,
    selected: props.selector.options.filter(option => next.includes(String(option.value))),
    nativeEvent,
  })
}

function choose(option: ChatComposerSelectorOption, nativeEvent: MouseEvent | KeyboardEvent) {
  if (props.selector.disabled || option.disabled) return
  const value = String(option.value)
  if (props.selector.multiple) {
    publish(values.value.includes(value) ? values.value.filter(item => item !== value) : [...values.value, value], nativeEvent)
  } else {
    publish([value], nativeEvent)
    root.value?.removeAttribute('open')
  }
}

function clear(nativeEvent: MouseEvent) {
  nativeEvent.preventDefault()
  nativeEvent.stopPropagation()
  publish([], nativeEvent)
}
</script>

<template>
  <details v-if="selector.visible !== false" ref="root" class="jcr-composer-selector"
    :class="{ 'is-disabled': selector.disabled }" :data-selector-id="selector.id">
    <summary :aria-label="selector.label" :aria-disabled="selector.disabled || undefined"
      @click="selector.disabled && $event.preventDefault()">
      <span>{{ summary }}</span>
      <button v-if="selector.clearable && selected.length" type="button" :aria-label="`Clear ${selector.label}`" @click="clear">×</button>
      <i aria-hidden="true">⌄</i>
    </summary>
    <div class="jcr-composer-selector__popup">
      <label v-if="selector.searchable" class="jcr-composer-selector__search">
        <span class="jcr-sr-only">Search {{ selector.label }}</span>
        <input v-model="query" type="search" :placeholder="selector.placeholder || selector.label" />
      </label>
      <div v-if="groups.length" class="jcr-composer-selector__options">
        <section v-for="group in groups" :key="group.label || '__default'">
          <small v-if="group.label">{{ group.label }}</small>
          <button v-for="option in group.options" :key="option.value" type="button"
            :disabled="option.disabled" :class="{ 'is-selected': values.includes(String(option.value)) }"
            @click="choose(option, $event)">
            <span v-if="option.icon" class="jcr-composer-selector__icon">{{ option.icon }}</span>
            <span><strong>{{ option.label }}</strong><small v-if="option.description">{{ option.description }}</small></span>
            <i aria-hidden="true">{{ values.includes(String(option.value)) ? '✓' : '' }}</i>
          </button>
        </section>
      </div>
      <p v-else class="jcr-composer-selector__empty">No options</p>
    </div>
  </details>
</template>
