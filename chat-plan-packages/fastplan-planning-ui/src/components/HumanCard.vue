<script setup>
import { ref, watch, computed, onUnmounted } from 'vue';
import { UserCheck, Clock, Send, Calendar, DollarSign, MessageSquare, X, CheckCircle2 } from 'lucide-vue-next';

const props = defineProps({
  request: {
    type: Object,
    required: true
  }
});

const emit = defineEmits(['response']);

const inputValue = ref("");
const timeLeft = ref(0);
let timer = null;

// 解析选项：后端可能返回字符串数组，也可能返回对象数组
const normalizedOptions = computed(() => {
  if (!props.request?.options) return [];
  return props.request.options.map(opt => {
    if (typeof opt === 'string') return { label: opt, value: opt };
    return {
      label: opt.label || opt.value,
      value: opt.value,
      isDefault: opt.is_default
    };
  });
});

// 智能判断输入类型
const inputType = computed(() => {
  if (!props.request) return 'none';
  const p = (props.request.prompt || '').toLowerCase();

  // 1. 强制日期类型
  if (p.includes("日期") || p.includes("date") || p.includes("时间") || p.includes("when")) return "date";

  // 2. 强制金额/数字类型
  if (p.includes("预算") || p.includes("budget") || p.includes("多少钱") || p.includes("金额") || p.includes("price") || p.includes("人数")) return "number";

  // 3. 纯选项类型：只有在不包含上述关键词，且有选项，且选项数量大于2（非简单的继续/取消）时才认为是纯选项
  // 或者如果选项明确不是 continue/abort 这种控制类的
  const isControlOption = normalizedOptions.value.some(o => o.value === 'continue' || o.value === 'abort');
  if (normalizedOptions.value.length > 0 && !isControlOption && !p.includes("确认") && !p.includes("输入")) {
    return "options";
  }

  // 4. 默认：文本输入 (包含"确认出发城市"这种场景，虽然有continue选项，但本质是文本输入)
  return "text";
});

const placeholder = computed(() => {
  switch (inputType.value) {
    case 'date': return '请选择日期...';
    case 'number': return '请输入数字/金额...';
    default: return '请输入您的回复...';
  }
});

const inputIcon = computed(() => {
  switch (inputType.value) {
    case 'date': return Calendar;
    case 'number': return DollarSign;
    default: return MessageSquare;
  }
});

// 倒计时逻辑
const progressPercent = computed(() => {
  if (!props.request) return 0;
  const total = props.request.timeout_seconds || 60;
  return Math.min(100, Math.max(0, (timeLeft.value / total) * 100));
});

const progressColor = computed(() => {
  if (progressPercent.value > 50) return 'bg-blue-500';
  if (progressPercent.value > 20) return 'bg-yellow-500';
  return 'bg-red-500';
});

// --- Timer & Watcher ---

const stopTimer = () => {
  if (timer) {
    clearInterval(timer);
    timer = null;
  }
};

const startTimer = () => {
  stopTimer();
  timer = setInterval(() => {
    if (timeLeft.value > 0) {
      timeLeft.value--;
    } else {
      handleSubmit({ action: 'continue', timeout: true });
    }
  }, 1000);
};

// 监听请求变化，重置状态
watch(() => props.request, (newVal) => {
  stopTimer();
  if (newVal) {
    inputValue.value = "";
    timeLeft.value = newVal.timeout_seconds || 60;
    startTimer();
  }
}, { immediate: true, deep: true });

onUnmounted(() => {
  stopTimer();
});

// --- 交互处理 ---

const handleSubmit = (val) => {
  stopTimer();

  let responseData = {};

  if (val && typeof val === 'object' && val.action) {
    // 已经是完整对象 (例如超时自动提交)
    responseData = val;
  } else if (val && typeof val === 'object' && val.value) {
    // 点击了选项按钮 (传入的是 option 对象)
    responseData = { action: val.value, value: val.label };
  } else if (typeof val === 'string') {
    // 简单的字符串 action
    responseData = { action: val, value: val };
  } else {
    // 默认：文本框提交
    // 如果是 "text" 类型，通常对应后端的 "continue" 动作，并附带 value
    const defaultAction = normalizedOptions.value.find(o => o.isDefault)?.value || 'continue';
    responseData = { action: defaultAction, value: inputValue.value };
  }

  emit('response', responseData);
};

// 处理取消
const handleCancel = () => {
  // 查找是否有名为 "abort" 或 "cancel" 的选项，如果没有则默认传 "abort"
  const cancelAction = normalizedOptions.value.find(o => o.value === 'abort' || o.value === 'cancel')?.value || 'abort';
  handleSubmit({ action: cancelAction, value: '已取消' });
};

const handleKeydown = (e) => {
  if (e.key === 'Enter' && !e.shiftKey) {
    if (inputType.value !== 'options' && inputValue.value.trim()) {
      e.preventDefault();
      handleSubmit();
    }
  }
};
</script>

<template>
  <transition
    enter-active-class="transition-all duration-300 ease-out"
    leave-active-class="transition-all duration-200 ease-in"
    enter-from-class="opacity-0 translate-y-4"
    enter-to-class="opacity-100 translate-y-0"
    leave-from-class="opacity-100 translate-y-0"
    leave-to-class="opacity-0 translate-y-4"
  >
    <div
      v-if="request"
      class="fpp-ui bg-white shadow-sm border-l-4 border-purple-500 p-6 rounded-r-xl w-full relative overflow-hidden animate-slideUp mb-4"
    >
      <!-- 右上角关闭/取消按钮 -->
      <button
        @click="handleCancel"
        class="absolute top-3 right-3 p-1.5 rounded-full hover:bg-gray-100 text-gray-400 hover:text-red-500 transition-colors"
        title="取消任务"
      >
        <X :size="16" />
      </button>

      <div class="flex gap-5 items-start">
         <!-- 左侧图标 (可选，使用 UserCheck 或其他) -->
         <div class="hidden sm:flex w-10 h-10 rounded-full bg-purple-50 text-purple-600 items-center justify-center flex-shrink-0 mt-1">
            <UserCheck :size="20" />
         </div>

         <!-- 右侧内容 -->
         <div class="flex-1 min-w-0">
             <!-- 提示语 -->
             <div class="mb-4">
                 <h4 class="font-bold text-gray-900 text-base mb-2">需要人工确认</h4>
                 <p class="text-gray-600 text-sm leading-relaxed">{{ request.prompt }}</p>
             </div>

             <!-- 交互区域 -->
             <div>
                <!-- 模式 A: 纯选项 -->
                <div v-if="inputType === 'options'" class="flex flex-wrap gap-3">
                  <button
                    v-for="(option, idx) in normalizedOptions"
                    :key="idx"
                    @click="handleSubmit(option)"
                    class="px-5 py-2.5 text-sm font-medium rounded-lg transition-all shadow-sm active:scale-95 flex items-center gap-2"
                    :class="option.value === 'continue' || option.isDefault
                      ? 'bg-purple-600 text-white hover:bg-purple-700'
                      : 'bg-white border border-gray-300 text-gray-700 hover:bg-gray-50'"
                  >
                    {{ option.label }}
                  </button>
                </div>

                <!-- 模式 B: 输入框 -->
                <div v-else class="flex flex-col gap-3 max-w-xl">
                   <!-- 默认快捷回复 (保留，但样式微调) -->
                   <div class="flex flex-wrap gap-2">
                      <button
                        v-for="quickReply in ['确认无误', '继续执行', '需修改']"
                        :key="quickReply"
                        @click="inputValue = quickReply; handleSubmit()"
                        class="px-3 py-1 bg-gray-50 border border-gray-200 hover:border-purple-300 hover:bg-purple-50 text-gray-600 hover:text-purple-600 text-xs rounded-full transition-all"
                      >
                        {{ quickReply }}
                      </button>
                   </div>

                   <div class="flex gap-3">
                      <input
                          :type="inputType === 'date' ? 'date' : (inputType === 'number' ? 'number' : 'text')"
                          v-model="inputValue"
                          @keydown="handleKeydown"
                          :placeholder="placeholder"
                          class="flex-1 px-4 py-2.5 bg-gray-50 border border-gray-200 rounded-lg text-sm focus:ring-2 focus:ring-purple-500/20 focus:border-purple-500 outline-none transition-all focus:bg-white"
                          ref="inputRef"
                          autofocus
                        />
                        <button
                          @click="handleSubmit()"
                          :disabled="!inputValue"
                          class="bg-purple-600 text-white px-6 py-2.5 rounded-lg hover:bg-purple-700 disabled:opacity-50 text-sm font-medium transition-colors shadow-sm"
                        >
                          确认执行
                        </button>
                   </div>
                </div>

                <!-- 倒计时与 ID (简化显示) -->
                <div class="flex items-center gap-3 mt-3 text-[10px] text-gray-400 font-mono">
                    <span v-if="timeLeft > 0" class="flex items-center gap-1 text-purple-500">
                      <Clock :size="10" />
                      {{ timeLeft }}s 后自动确认
                    </span>
                    <span v-else class="text-orange-500">已暂停等待</span>
                </div>
             </div>
         </div>
      </div>
    </div>
  </transition>
</template>

<style scoped>
input[type="number"]::-webkit-inner-spin-button,
input[type="number"]::-webkit-outer-spin-button {
  -webkit-appearance: none;
  margin: 0;
}
</style>
