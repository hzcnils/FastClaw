<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useWebSearchStore, PROVIDERS } from '@/stores/websearch'
import { ElMessage } from 'element-plus'
import { Eye, EyeOff } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import SvgIcon from '@/components/svgIcon/index.vue'

const { t } = useI18n()
const ws = useWebSearchStore()
onMounted(() => ws.load())

const c = ws.cfg
const testing = ref(false)
const newDomain = ref('')
const visibleKeys = ref<Record<string, boolean>>({})

function toggleVisible(key: string) {
  visibleKeys.value[key] = !visibleKeys.value[key]
}
const cur = computed(() => PROVIDERS.find((p) => p.id === c.value.provider))

async function runTest() {
  testing.value = true
  try {
    const hits = await ws.test('FastClaw 联网搜索测试')
    ElMessage.success(t('chat.wsTestSuccess', { n: hits.length }))
  } catch (e: any) {
    ElMessage.error(t('chat.wsTestFail', { msg: e?.message || e }))
  } finally {
    testing.value = false
  }
}

function addBlacklist() {
  const d = newDomain.value.trim()
  if (!d) return
  if (!c.value.blacklist.includes(d)) ws.patch({ blacklist: [...c.value.blacklist, d] })
  newDomain.value = ''
}
function removeBlacklist(d: string) {
  ws.patch({ blacklist: c.value.blacklist.filter((x) => x !== d) })
}

async function onSaveClick() {
  try {
    await ws.save()
    ElMessage.success(t('chat.wsSaveSuccess'))
  } catch (e: any) {
    ElMessage.error(t('chat.wsSaveFail', { msg: e?.message || e }))
  }
}
</script>

<template>
  <div class="wss">
    <div class="wss-inner">
    <div class="wss-title">
      <el-icon :size="20"><svg-icon iconName="internet" /></el-icon>
      {{ $t('chat.webSearch') }}
    </div>
    <div class="wss-sub">{{ $t('chat.wsDesc') }}</div>

    <div class="wss-toprow">
      <div>
        <div class="t">{{ $t('chat.wsEnable') }}</div>
        <div class="d">{{ $t('chat.wsEnableDesc') }}</div>
      </div>
      <el-switch v-model="c.enabled" @change="ws.save()" />
    </div>

    <div class="wss-sec">{{ $t('chat.wsProvider') }}</div>
    <div class="wss-grid">
      <div
        v-for="p in ws.availableProviders"
        :key="p.id"
        class="wss-prov"
        :class="{ sel: c.provider === p.id }"
        @click="c.provider = p.id; ws.save()"
      >
        <span class="ico" :style="{ background: p.color }">{{ p.icon }}</span>
        <div class="pn">
          <div class="nm">{{ p.name }}</div>
          <div v-if="p.id === 'google' || p.id === 'bing'" class="sub">{{ $t('chat.wsUnstable') }}</div>
        </div>
        <span class="badge" :class="p.badge === 'free' ? 'free' : (p.ready ? 'ok' : 'key')">
          {{ p.badge === 'free' ? $t('chat.wsFree') : (p.ready ? $t('chat.wsConfigured') : $t('chat.wsNeedKey')) }}
        </span>
      </div>
    </div>

    <template v-if="c.provider === 'bocha'">
      <div class="wss-sec">{{ $t('chat.wsApiConfig') }}（博查 Bocha）</div>
      <div class="wss-field">
        <div class="lbl">API Key</div>
        <div class="key-wrap">
          <input :type="visibleKeys['bocha'] ? 'text' : 'password'" class="inp" v-model="c.keys.bocha" placeholder="sk-..." @change="ws.save()" />
          <button class="key-eye" @click="toggleVisible('bocha')" type="button">
            <Eye v-if="!visibleKeys['bocha']" :size="16" />
            <EyeOff v-else :size="16" />
          </button>
        </div>
      </div>
    </template>
    <template v-if="c.provider === 'tavily'">
      <div class="wss-sec">{{ $t('chat.wsApiConfig') }}（Tavily）</div>
      <div class="wss-field">
        <div class="lbl">API Key</div>
        <div class="key-wrap">
          <input :type="visibleKeys['tavily'] ? 'text' : 'password'" class="inp" v-model="c.keys.tavily" placeholder="tvly-..." @change="ws.save()" />
          <button class="key-eye" @click="toggleVisible('tavily')" type="button">
            <Eye v-if="!visibleKeys['tavily']" :size="16" />
            <EyeOff v-else :size="16" />
          </button>
        </div>
      </div>
    </template>
    <template v-else-if="c.provider === 'exa'">
      <div class="wss-sec">{{ $t('chat.wsApiConfig') }}（Exa）</div>
      <div class="wss-field">
        <div class="lbl">API Key</div>
        <div class="key-wrap">
          <input :type="visibleKeys['exa'] ? 'text' : 'password'" class="inp" v-model="c.keys.exa" placeholder="exa-..." @change="ws.save()" />
          <button class="key-eye" @click="toggleVisible('exa')" type="button">
            <Eye v-if="!visibleKeys['exa']" :size="16" />
            <EyeOff v-else :size="16" />
          </button>
        </div>
      </div>
    </template>

    <div class="wss-sec">{{ $t('chat.wsParams') }}</div>
    <div class="wss-params">
      <div>
        <div class="lbl">{{ $t('chat.wsMaxResults') }}：{{ c.maxResults }}</div>
        <el-slider v-model="c.maxResults" :min="1" :max="10" @change="ws.save()" />
      </div>
      <div>
        <div class="lbl">{{ $t('chat.wsTimeRange') }}</div>
        <el-radio-group v-model="c.timeRange" size="small" @change="ws.save()">
          <el-radio-button value="none">{{ $t('chat.wsAny') }}</el-radio-button>
          <el-radio-button value="year">{{ $t('chat.wsYear') }}</el-radio-button>
          <el-radio-button value="month">{{ $t('chat.wsMonth') }}</el-radio-button>
        </el-radio-group>
      </div>
      <div>
        <div class="lbl">{{ $t('chat.wsSafe') }}</div>
        <el-switch v-model="c.safeSearch" @change="ws.save()" />
      </div>
    </div>

    <div class="wss-sec">{{ $t('chat.wsInject') }}</div>
    <el-radio-group v-model="c.inject" @change="ws.save()">
      <el-radio-button value="system">{{ $t('chat.wsInjectSystem') }}</el-radio-button>
      <el-radio-button value="cite">{{ $t('chat.wsInjectCite') }}</el-radio-button>
    </el-radio-group>

    <div class="wss-sec">{{ $t('chat.wsBlacklist') }}</div>
    <div class="wss-tags">
      <span v-for="d in c.blacklist" :key="d" class="tag">
        {{ d }} <i @click="removeBlacklist(d)">✕</i>
      </span>
      <form @submit.prevent="addBlacklist" style="display:inline;">
        <input
          class="tag-input"
          v-model="newDomain"
          :placeholder="$t('chat.wsAddDomain')"
        />
      </form>
    </div>

    <div class="wss-foot">
      <el-button type="primary"  @click="runTest" :loading="testing">{{ $t('chat.wsTest') }}</el-button>
      <el-button type="primary" @click="onSaveClick">{{ $t('common.save') }}</el-button>
    </div>
    </div>
  </div>
</template>

<style scoped>
.wss { height: 100%; display: flex; flex-direction: column; padding: 0; }
.wss-inner { flex: 1; overflow-y: auto; }
.wss-inner::-webkit-scrollbar { width: 6px; }
.wss-inner::-webkit-scrollbar-thumb { background: var(--border-color); border-radius: 3px; }
.wss-title {
  font-size: var(--font-size-xl); font-weight: 700; color: var(--text-primary);
  display: flex; align-items: center; gap: 9px;
}
.wss-title .el-icon { color: var(--color-primary); }
.wss-sub { font-size: var(--font-size-md); color: var(--text-secondary); margin: 6px 0 14px; }
.wss-sec { font-size: var(--font-size-md); font-weight: 700; color: var(--text-secondary); margin: 16px 0 10px; }
.wss-toprow {
  display: flex; align-items: center; justify-content: space-between;
  padding: 10px 10px; 
  /* background: var(--bg-hover); */
  border: 1px solid var(--border-color); border-radius: 12px;
}
.wss-toprow .t { font-size: var(--font-size-base); font-weight: 600; color: var(--text-primary); }
.wss-toprow .d { font-size: var(--font-size-sm); color: var(--text-secondary); margin-top: 2px; }
.wss-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; max-height: 200px; overflow-y: auto; padding-right: 4px; }
.wss-grid::-webkit-scrollbar { width: 6px; }
.wss-grid::-webkit-scrollbar-thumb { background: var(--border-color); border-radius: 3px; }
.wss-prov {
  display: flex; align-items: center; gap: 11px; padding: 12px 14px;
  border: 1.5px solid var(--border-color); border-radius: 11px; cursor: pointer;
  transition: all 0.15s;
}
.wss-prov.sel { border-color: var(--color-primary); background: rgba(79, 107, 237, 0.05); }
.wss-prov .ico {
  width: 28px; height: 28px; border-radius: 8px;
  display: flex; align-items: center; justify-content: center;
  font-weight: 700; font-size: var(--font-size-base); color: #fff; flex-shrink: 0;
}
.wss-prov .nm { font-size: var(--font-size-base); font-weight: 600; color: var(--text-primary); }
.wss-prov .sub { font-size: var(--font-size-sm); color: var(--text-secondary); margin-top: 1px; }
.badge { font-size: var(--font-size-sm); padding: 2px 8px; border-radius: 6px; font-weight: 600; margin-left: auto; }
.badge.free { color: #15a34a; background: #ecfdf3; border: 1px solid #b9eccb; }
.badge.key { color: #ea7517; background: #fff5eb; border: 1px solid #ffdcb8; }
.badge.ok { color: var(--color-primary); background: rgba(79, 107, 237, 0.08); border: 1px solid rgba(79, 107, 237, 0.2); }
.wss-field { margin-top: 8px; }
.lbl { font-size: var(--font-size-md); color: var(--text-primary); margin-bottom: 6px; font-weight: 500; }
.inp {
  width: 100%; border: 1px solid var(--border-color); border-radius: 9px;
  padding: 9px 12px; font-size: var(--font-size-md); color: var(--text-primary);
  outline: none;
   /* background: var(--bg-primary);  */
   font-family: ui-monospace, monospace;
}
.inp:focus { border-color: var(--color-primary); }
.key-wrap {
  position: relative;
  display: flex;
  align-items: center;
}
.key-wrap .inp {
  padding-right: 38px;
}
.key-eye {
  position: absolute;
  right: 8px;
  top: 50%;
  transform: translateY(-50%);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 4px;
  border: none;
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  border-radius: 6px;
  transition: color 0.15s, background 0.15s;
}
.key-eye:hover {
  color: var(--color-primary);
  background: var(--bg-hover);
}
.wss-params { display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 18px; align-items: start; }
.wss-params :deep(.el-radio-button__inner) {
  font-size: var(--font-size-sm);
}
.wss-tags { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; }
.tag {
  display: inline-flex; align-items: center; gap: 6px; font-size: var(--font-size-sm);
  background: var(--bg-hover); border-radius: 7px; padding: 5px 10px; color: var(--text-secondary);
}
.tag i { cursor: pointer; font-style: normal; opacity: 0.6; }
.tag i:hover { opacity: 1; color: #f43f5e; }
.tag-input {
  border: 1px dashed var(--border-color); border-radius: 7px; padding: 5px 10px;
  font-size: var(--font-size-sm); outline: none; background: transparent; color: var(--text-primary); width: calc(var(--font-size-base) * 10);
}
.wss-foot { display: flex; justify-content: flex-end; gap: 10px; margin-top: 18px; }
</style>
