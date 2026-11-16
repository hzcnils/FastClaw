<script setup lang="ts">
import { watch, onMounted, onUnmounted, computed } from 'vue';

const props = withDefaults(defineProps<{
  modelValue?: boolean;
  show?: boolean;
  title?: string;
  size?: 'sm' | 'md' | 'lg' | 'xl' | '2xl' | 'full';
  closeOnOverlay?: boolean;
  showClose?: boolean;
  contentStyle?: Record<string, string>;
}>(), {
  size: 'md',
  closeOnOverlay: false,
  showClose: true,
});

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void;
  (e: 'update:show', value: boolean): void;
  (e: 'close'): void;
}>();

const sizeClasses = {
  sm: 'max-w-sm',
  md: 'max-w-lg',
  lg: 'max-w-2xl',
  xl: 'max-w-4xl',
  '2xl': 'max-w-6xl',
  full: 'max-w-[90vw]',
};

const visible = computed({
  get: () => {
    // Boolean prop 未传入时运行时是 false 而不是 undefined，
    // 不能用 `??` / `!== undefined`：否则仅传 :show 时会被 modelValue=false 短路，弹窗永远不出现。
    return Boolean(props.modelValue || props.show);
  },
  set: (value: boolean) => {
    emit('update:modelValue', value);
    emit('update:show', value);
  },
});

function closeModal() {
  visible.value = false;
  emit('close');
}

function handleOverlayClick() {
  if (props.closeOnOverlay !== true) return;
  closeModal();
}

function handleEscape(e: KeyboardEvent) {
  if (e.key === 'Escape' && visible.value) {
    closeModal();
  }
}

watch(() => visible.value, (show) => {
  if (show) {
    document.body.style.overflow = 'hidden';
  } else {
    document.body.style.overflow = '';
  }
});

onMounted(() => {
  document.addEventListener('keydown', handleEscape);
});

onUnmounted(() => {
  document.removeEventListener('keydown', handleEscape);
  document.body.style.overflow = '';
});
</script>

<template>
  <Teleport to="body">
    <Transition name="modal">
      <div v-if="visible" class="modal-overlay" @click.self="handleOverlayClick">
        <div :class="['modal-content w-full', sizeClasses[size]]" @click.stop>
          <!-- Header -->
          <div v-if="title || showClose" class="modal-header">
            <h3 v-if="title" class="text-lg font-semibold text-slate-900 dark:text-slate-100">{{ title }}</h3>
            <button 
              v-if="showClose"
              @click="closeModal()"
              class="p-2 -mr-2 rounded-lg  dark:hover:text-slate-100 hover:bg-[var(--dropdown-hover-color)] dark:hover:bg-slate-700 transition-colors"
            >
              <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          </div>

          <!-- Body -->
          <div class="modal-body">
            <slot />
          </div>

          <!-- Footer -->
          <div v-if="$slots.footer" class="modal-footer">
            <slot name="footer" />
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.modal-enter-active,
.modal-leave-active {
  transition: opacity 0.2s ease;
}

/* .modal-enter-active .modal-content,
.modal-leave-active .modal-content {
  transition: transform 0.2s ease, opacity 0.2s ease;
  
} */

.modal-enter-from,
.modal-leave-to {
  opacity: 0;
}

/* .modal-enter-from .modal-content,
.modal-leave-to .modal-content {
  transform: scale(0.95);
  opacity: 0;
  background-color: transparent !important; 
  
} */
 .modal-content {
  background-color: var(--theme-dailog-color);  
  background:  var(--theme-dailog-color);
} 

.modal-overlay {
  position: fixed;
  inset: 0;
  background-color: rgba(0, 0, 0, 0.15);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 10050;
  backdrop-filter: blur(0px);
}

</style>
