<template>
  <el-container style="padding-top: 25px; padding-right: 0px; padding-bottom: 0px; padding-left: 0px;height: 100%; background: var(--bg-primary);" :data-theme-id="themeStore.draftPreset.id">
    <jmisSearch ref="jmisSearchRef" :searchItem="searchItem" @search="inquires" @reset="reset">
      <template #button>
        <el-button @click="openColSetting($event)">{{ $t('dashboard.columnSetting') }}</el-button>
      </template>
    </jmisSearch>

    <el-header height="auto" class="custom-header">
      <div class="left-panel">
        <jmisButton :listItem="listItem" @buttonClick="buttonClick" />
      </div>
      <div class="right-panel">
        <el-tag type="primary">{{ $t('dashboard.consumptionQuota') }}{{ consumptionQuota }}</el-tag>
        <el-tag type="warning">{{ $t('dashboard.rpm') }}{{ rpm }}</el-tag>
        <el-tag type="info">{{ $t('dashboard.tpm') }}{{ tpm }}</el-tag>
      </div>
    </el-header>
    <el-main class="nopadding">
      <jmisTable :size="selected.length" :total="total" :data="tableData" :column="column" @selectionChange="selectionChangeHandle"
        ref="jmisTableRef" :autoPageSize="false" @pageChange="pageChangeFn">
        <el-table-column type="selection" width="50" fixed :reserve-selection="true" />
        <el-table-column type="expand">
          <template #default="{ row }">
            <el-descriptions title="" :column="1" border :label-width="110">
              <template v-for="(item, index) in row.expandData" :key="index">
                <el-descriptions-item :label="item.key" label-align="right">
                  <span v-if="typeof item.value == 'string'">{{ item.value }}</span>
                  <div v-else-if="Array.isArray(item.value)">
                    <div v-for="(child, chind) in item.value" :key="chind">
                      {{ child }}
                    </div>
                  </div>
                </el-descriptions-item>
              </template>
            </el-descriptions>
          </template>
        </el-table-column>

        <template #channel="{ row }">
          <span v-if="isAdminUser && [0, 2, 5, 6].includes(row.type)">
            <el-space>
              <span style="position: relative; display: inline-block">
                <el-tooltip :content="row.channel_name || $t('dashboard.unknownChannel')">
                  <el-tag size="small" effect="plain">{{ row.channel }}</el-tag>
                </el-tooltip>
                <span v-if="getLogOther(row.other)?.admin_info?.channel_affinity" style="position: absolute; right: -4px; top: -4px; color: #f59e0b; cursor: pointer;" @click.stop="openChannelAffinityUsageCacheModal(getLogOther(row.other).admin_info.channel_affinity)">
                  <el-tooltip>
                    <template #content>
                      <div style="line-height: 1.6">
                        <div>{{ $t('dashboard.channel') }}：{{ getLogOther(row.other)?.admin_info?.use_channel?.join('->') || row.channel }}</div>
                      </div>
                    </template>
                    <StarFilled style="width: 14px; height: 14px" />
                  </el-tooltip>
                </span>
              </span>
              <el-tag v-if="getLogOther(row.other)?.admin_info?.is_multi_key" size="small" effect="light">
                {{ getLogOther(row.other).admin_info.multi_key_index }}
              </el-tag>
            </el-space>
          </span>
        </template>
        <template #created_at="{ row }">
          <span>{{ timeFormat(row.created_at) }}</span>
        </template>
        <template #type="{ row }">
          <span>{{ typeFormat(row.type) }}</span>
        </template>
        <template #group="{ row }">
          <span v-if="[0, 2, 5, 6].includes(row.type)">
            <span v-if="row.group">{{ renderGroup(row.group) }}</span>
            <span v-else>{{ renderGroup(getLogOther(row.other)?.group) }}</span>
          </span>
        </template>
        <template #model_name="{ row }">
          <div v-if="[0, 2, 5, 6].includes(row.type)">
            <div v-if="!isModelMapped(row)">
              <el-tag class="model-tag" @click.stop="copyText(row.model_name)">{{ row.model_name }}</el-tag>
            </div>
            <el-popover v-else placement="top" width="300" trigger="hover">
              <div class="p-2">
                <div class="mb-1"><span class="font-bold mr-2">{{ $t('dashboard.requestedBillingModel') }}</span><el-tag @click.stop="copyText(row.model_name)">{{ row.model_name }}</el-tag></div>
                <div><span class="font-bold mr-2">{{ $t('dashboard.actualModel') }}</span><el-tag @click.stop="copyText(getLogOther(row.other).upstream_model_name)">{{ getLogOther(row.other).upstream_model_name }}</el-tag></div>
              </div>
              <el-tag class="model-tag" append-to-body @click.stop="copyText(row.model_name)">{{ row.model_name }}<svg-icon name="Route" width="12" height="12" class="ml-1 opacity-75" /></el-tag>
            </el-popover>
          </div>
        </template>
        <template #use_time="{ row }">
          <div v-if="[2, 5].includes(row.type)">
            <el-space>
              <el-tag round :type="getTimeTagType(row.use_time)">{{ row.use_time }}s</el-tag>
              <el-tag v-if="row.is_stream" round :type="getFirstTokenTagType(getLogOther(row.other)?.frt)">{{ (Number(getLogOther(row.other)?.frt || 0) / 1000).toFixed(1) }}s</el-tag>
              <el-tag round :type="row.is_stream ? 'primary' : 'purple'">{{ row.is_stream ? $t('dashboard.stream') : $t('dashboard.nonStream') }}</el-tag>
            </el-space>
          </div>
        </template>
        <template #prompt_tokens="{ row }">
          <div v-if="[0, 2, 5, 6].includes(row.type)">
            <span>{{ formatToken(row.prompt_tokens) }}</span>
            <div v-if="getCacheSummary(row.other)" class="text-xs text-gray-500 mt-1">{{ getCacheSummary(row.other) }}</div>
          </div>
        </template>
        <template #completion_tokens="{ row }">
          <span v-if="Number(row.completion_tokens) > 0 && [0, 2, 5, 6].includes(row.type)">{{ formatToken(row.completion_tokens) }}</span>
        </template>
        <template #quota="{ row }">
          <div v-if="[0, 2, 5, 6].includes(row.type)">
            <el-tooltip v-if="isSubscriptionBilled(row.other)" :content="$t('dashboard.deductedBySubscription') + renderQuota(row.quota, 6)" placement="top">
              <el-tag type="success" round>{{ $t('dashboard.subscriptionDeduction') }}</el-tag>
            </el-tooltip>
            <span v-else>{{ renderQuota(row.quota, 6) }}</span>
          </div>
        </template>
        <template #ip="{ row }">
          <el-tooltip v-if="[2, 5].includes(row.type) && row.ip" :content="row.ip" placement="top">
            <el-tag type="warning" round @click.stop="copyText(row.ip)">{{ row.ip }}</el-tag>
          </el-tooltip>
        </template>
        <template #retry="{ row }">
          <div v-if="isAdminUser && [2, 5].includes(row.type)">{{ getChannelPath(row) }}</div>
        </template>
        <el-table-column :label="$t('dashboard.detail')" fixed="right" width="150">
          <template #default="{ row }">
            <div class="detail-cell">
              <div v-if="getUsageLogDetailSummary(row, row.content)">
                <div v-for="(seg, idx) in getUsageLogDetailSummary(row, row.content).segments" :key="idx" :class="['text-base', seg.tone === 'secondary' ? 'text-gray-400' : 'text-gray-700']" class="truncate" :title="seg.text">{{ seg.text }}</div>
              </div>
              <el-tooltip v-else :content="row.content" placement="top" :max-width="300">
                <div class="text-base truncate">{{ row.content }}</div>
              </el-tooltip>
            </div>
          </template>
        </el-table-column>
      </jmisTable>
    </el-main>

    <save-dialog v-dialogDrag v-if="dialog.save" ref="saveDialogRef" @success="handleSuccess" @data="submissionData" @closed="dialog.save = false" />
  </el-container>
</template>

<script setup>
import { ref, reactive, onMounted, nextTick, inject, computed } from "vue";
import { useI18n } from 'vue-i18n';
import {
  ElContainer, ElHeader, ElMain, ElMessage, ElTableColumn, ElTag,
  ElDescriptions, ElDescriptionsItem, ElTooltip, ElSpace,
} from "element-plus";
import { Delete, ZoomIn, Upload, StarFilled } from "@element-plus/icons-vue";
import { useThemeStore } from '@/stores/theme';
import {
  renderClaudeLogContent, renderLogContent, renderAudioModelPrice,
  renderClaudeModelPrice, renderModelPrice, renderModelPriceSimple,
} from '@/views/render';
import jmisTable from "@/components/jmisTable/index.vue";
import jmisSearch from "@/components/jmisSearch/index.vue";
import jmisButton from "@/components/jmisButton/index.vue";
import saveDialog from "@/views/useLog/save.vue";
import newApi from "@/api/newApi";

const themeStore = useThemeStore()
const { t } = useI18n()
const $API = inject("$API");
const $TOOL = inject("$TOOL");
const column = computed(() => [
  { label: t("dashboard.colTime"), prop: "created_at", width: 150 },
  { label: t("dashboard.colChannel"), prop: "channel", width: 60 },
  { label: t("dashboard.colUser"), prop: "username" },
  { label: t("dashboard.colToken"), prop: "token_name" },
  { label: t("dashboard.colGroup"), prop: "group" },
  { label: t("dashboard.colType"), prop: "type" },
  { label: t("dashboard.colModel"), prop: "model_name", width: 190 },
  { label: t("dashboard.colUseTime"), prop: "use_time", width: 150 },
  { label: t("dashboard.colInput"), prop: "prompt_tokens" },
  { label: t("dashboard.colOutput"), prop: "completion_tokens" },
  { label: t("dashboard.colCost"), prop: "quota", width: 90 },
  { label: t("dashboard.colIP"), prop: "ip" },
  { label: t("dashboard.colRetry"), prop: "retry" },
]);

const jmisSearchRef = ref(null);
const isAdminUser = ref(true);
const apiObj = newApi.useLogApi;
const BILLING_DISPLAY_MODE_STORAGE_KEY = computed(() => {
  return isAdminUser.value ? 'newapi_logs-billing-display-mode-admin' : 'newapi_logs-billing-display-mode-user';
})
const tableParams = reactive({
  type: 0,
  start_timestamp: new Date().setHours(0, 0, 0, 0).toString().substring(0, 10),
  end_timestamp: new Date().getTime().toString().substring(0, 10),
});
const selected = ref([]);
const dialog = reactive({ save: false });
const queryInfo = ref({
  type: '0', username: '', token_name: '', model_name: '',
  start_timestamp: new Date().setHours(0, 0, 0, 0).toString().substring(0, 10),
  end_timestamp: new Date().getTime().toString().substring(0, 10),
  channel: '', group: '',
});
const tableData = ref([]);
const total = ref(0);
const consumptionQuota = ref(0);
const rpm = ref(0);
const tpm = ref(0);
const typeList = computed(() => [
  { label: t("dashboard.typeAll"), value: "0" }, { label: t("dashboard.typeRecharge"), value: "1" }, { label: t("dashboard.typeConsumption"), value: "2" },
  { label: t("dashboard.typeManagement"), value: "3" }, { label: t("dashboard.typeSystem"), value: "4" }, { label: t("dashboard.typeError"), value: "5" }, { label: t("dashboard.typeRefund"), value: "6" },
])

const searchItem = computed(() => [
  { label: t("dashboard.colTime"), prop: "dateTime", type: "datetimerange" },
  { label: t("dashboard.searchTokenName"), prop: "token_name", type: "input" },
  { label: t("dashboard.searchModelName"), prop: "model_name", type: "input" },
  { label: t("dashboard.searchGroup"), prop: "group", type: "input" },
  { label: t("dashboard.searchChannelId"), prop: "channel", type: "input" },
  { label: t("dashboard.searchUsername"), prop: "username", type: "input" },
  { label: t("dashboard.searchLogType"), prop: "type", type: "select", selectItem: [...typeList.value], labelText: 'label', valueText: 'value' },
]);

const listItem = computed(() => [
  { name: t("dashboard.btnView"), type: "button", icon: ZoomIn, typeColor: "primary", index: 0 },
  {
    name: t("dashboard.btnExportSelected"), type: "down", icon: Upload, typeColor: "primary",
    searchItem: [
      { name: t("dashboard.btnClearSelection"), icon: Delete },
      { name: t("dashboard.btnExportAll"), icon: Upload },
    ],
    index: 0,
  },
]);

const jmisTableRef = ref(null);
const saveDialogRef = ref(null);
const current = ref(1);
const pageSize = ref(10);

const getData = () => {
  let req = { ...tableParams, ...queryInfo.value, p: current.value, page_size: pageSize.value }
  apiObj.get(req).then(res => {
    if (res.success) {
      let data = res.data.items;
      total.value = res.data.total;
      tableData.value = data.map(item => ({ ...item, expandData: setLogsFormat(item) }))
    }
  })
}
const pageChangeFn = (obj) => { current.value = obj.currentPage; pageSize.value = obj.pageSize; getData(); }
const getStatData = () => {
  let req = { ...tableParams, ...queryInfo.value }
  apiObj.getStat(req).then(res => {
    if (res.success) {
      let data = res.data;
      consumptionQuota.value = Number(numFormat(data.quota)).toFixed(2);
      rpm.value = data.rpm; tpm.value = data.tpm;
    }
  })
}
const requestConversionDisplayValue = (conversionChain) => {
  const chain = Array.isArray(conversionChain) ? conversionChain.filter(Boolean) : [];
  if (chain.length <= 1) return t('dashboard.nativeFormat');
  return `${chain.join(' -> ')}`;
};
const setLogsFormat = (item) => {
  let other = getLogOther(item.other);
  let expandDataLocal = [];
  if (isAdminUser.value && (item.type === 0 || item.type === 2 || item.type === 6)) {
    expandDataLocal.push({ key: t('dashboard.expandChannelInfo'), value: `${item.channel} - ${item.channel_name || t('dashboard.unknown')}` });
  }
  if (item.request_id) expandDataLocal.push({ key: 'Request ID', value: item.request_id });
  if (other?.ws || other?.audio) {
    expandDataLocal.push({ key: t('dashboard.expandVoiceInput'), value: other.audio_input }, { key: t('dashboard.expandVoiceOutput'), value: other.audio_output }, { key: t('dashboard.expandTextInput'), value: other.text_input }, { key: t('dashboard.expandTextOutput'), value: other.text_output });
  }
  if (other?.cache_tokens > 0) expandDataLocal.push({ key: t('dashboard.expandCacheTokens'), value: other.cache_tokens });
  if (other?.cache_creation_tokens > 0) expandDataLocal.push({ key: t('dashboard.expandCacheCreationTokens'), value: other.cache_creation_tokens });
  if (item.type === 2) {
    expandDataLocal.push({ key: t('dashboard.expandLogDetail'), value: other?.claude
      ? renderClaudeLogContent(other?.model_ratio, other.completion_ratio, other.model_price, other.group_ratio, other?.user_group_ratio, other.cache_ratio || 1.0, other.cache_creation_ratio || 1.0, other.cache_creation_tokens_5m || 0, other.cache_creation_ratio_5m || other.cache_creation_ratio || 1.0, other.cache_creation_tokens_1h || 0, other.cache_creation_ratio_1h || other.cache_creation_ratio || 1.0, getInitialBillingDisplayMode())
      : renderLogContent(other?.model_ratio, other.completion_ratio, other.model_price, other.group_ratio, other?.user_group_ratio, other.cache_ratio || 1.0, false, 1.0, other.web_search || false, other.web_search_call_count || 0, other.file_search || false, other.file_search_call_count || 0, getInitialBillingDisplayMode()),
    });
    if (item?.content) expandDataLocal.push({ key: t('dashboard.expandOtherDetail'), value: item.content });
    if (isAdminUser.value && other?.reject_reason) expandDataLocal.push({ key: t('dashboard.expandRejectReason'), value: other.reject_reason });
  }
  if (item.type === 2) {
    let modelMapped = other?.is_model_mapped && other?.upstream_model_name && other?.upstream_model_name !== '';
    if (modelMapped) { expandDataLocal.push({ key: t('dashboard.expandRequestedBillingModel'), value: item.model_name }, { key: t('dashboard.expandActualModel'), value: other.upstream_model_name }); }
    const isViolationFeeLog = other?.violation_fee === true || Boolean(other?.violation_fee_code) || Boolean(other?.violation_fee_marker);
    let content = '';
    if (!isViolationFeeLog) {
      if (other?.ws || other?.audio) {
        content = renderAudioModelPrice(other?.text_input, other?.text_output, other?.model_ratio, other?.model_price, other?.completion_ratio, other?.audio_input, other?.audio_output, other?.audio_ratio, other?.audio_completion_ratio, other?.group_ratio, other?.user_group_ratio, other?.cache_tokens || 0, other?.cache_ratio || 1.0, getInitialBillingDisplayMode());
      } else if (other?.claude) {
        content = renderClaudeModelPrice(item.prompt_tokens, item.completion_tokens, other.model_ratio, other.model_price, other.completion_ratio, other.group_ratio, other?.user_group_ratio, other.cache_tokens || 0, other.cache_ratio || 1.0, other.cache_creation_tokens || 0, other.cache_creation_ratio || 1.0, other.cache_creation_tokens_5m || 0, other.cache_creation_ratio_5m || other.cache_creation_ratio || 1.0, other.cache_creation_tokens_1h || 0, other.cache_creation_ratio_1h || other.cache_creation_ratio || 1.0, getInitialBillingDisplayMode());
      } else {
        content = renderModelPrice(item.prompt_tokens, item.completion_tokens, other?.model_ratio, other?.model_price, other?.completion_ratio, other?.group_ratio, other?.user_group_ratio, other?.cache_tokens || 0, other?.cache_ratio || 1.0, other?.image || false, other?.image_ratio || 0, other?.image_output || 0, other?.web_search || false, other?.web_search_call_count || 0, other?.web_search_price || 0, other?.file_search || false, other?.file_search_call_count || 0, other?.file_search_price || 0, other?.audio_input_seperate_price || false, other?.audio_input_token_count || 0, other?.audio_input_price || 0, other?.image_generation_call || false, other?.image_generation_call_price || 0, getInitialBillingDisplayMode());
      }
      expandDataLocal.push({ key: t('dashboard.expandBillingProcess'), value: content });
    }
    if (other?.reasoning_effort) expandDataLocal.push({ key: 'Reasoning Effort', value: other.reasoning_effort });
  }
  if (item.type === 6) {
    if (other?.task_id) expandDataLocal.push({ key: t('dashboard.expandTaskId'), value: other.task_id });
    if (other?.reason) expandDataLocal.push({ key: t('dashboard.expandFailReason'), value: other.reason });
  }
  if (other?.request_path) expandDataLocal.push({ key: t('dashboard.expandRequestPath'), value: other.request_path });
  if (other?.billing_source === 'subscription') {
    const planId = other?.subscription_plan_id, planTitle = other?.subscription_plan_title || '', subscriptionId = other?.subscription_id;
    const unit = t('dashboard.quotaUnit'), pre = other?.subscription_pre_consumed ?? 0, postDelta = other?.subscription_post_delta ?? 0;
    const finalConsumed = other?.subscription_consumed ?? pre + postDelta, remain = other?.subscription_remain, total = other?.subscription_total;
    if (planId) expandDataLocal.push({ key: t('dashboard.expandSubscriptionPlan'), value: t('dashboard.subscriptionInfo', { id: planId, title: planTitle }).trim() });
    if (subscriptionId) expandDataLocal.push({ key: t('dashboard.expandSubscriptionInstance'), value: `#${subscriptionId}` });
    expandDataLocal.push({ key: t('dashboard.expandSubscriptionSettlement'), value: [t('dashboard.subscriptionPreDeduct', { n: pre, unit }), t('dashboard.subscriptionSettlementDelta', { delta: `${postDelta > 0 ? '+' : ''}${postDelta}`, unit }), t('dashboard.subscriptionFinalDeduction', { n: finalConsumed, unit })].join('\n') });
    if (remain !== undefined && total !== undefined) expandDataLocal.push({ key: t('dashboard.expandSubscriptionRemain'), value: t('dashboard.subscriptionRemaining', { remain, total, unit }) });
  }
  if (isAdminUser && item.type !== 6) {
    expandDataLocal.push({ key: t('dashboard.expandRequestConversion'), value: requestConversionDisplayValue(other?.request_conversion) });
    expandDataLocal.push({ key: t('dashboard.expandBillingMode'), value: other?.admin_info?.local_count_tokens ? t('dashboard.billingLocal') : t('dashboard.billingUpstream') });
  }
  return expandDataLocal;
}
const getInitialBillingDisplayMode = () => {
  const savedMode = localStorage.getItem(BILLING_DISPLAY_MODE_STORAGE_KEY);
  if (savedMode === 'price' || savedMode === 'ratio') return savedMode;
  return localStorage.getItem('newapi_quota_display_type') === 'TOKENS' ? 'ratio' : 'price';
};
function getUsageLogDetailSummary(record, text) {
  const other = getLogOther(record.other);
  if (record.type === 6) return { segments: [{ text: t('dashboard.asyncTaskRefund'), tone: 'primary' }] };
  if (other == null || record.type !== 2) return null;
  if (other?.violation_fee === true || Boolean(other?.violation_fee_code) || Boolean(other?.violation_fee_marker)) {
    const feeQuota = other?.fee_quota ?? record?.quota;
    const groupText = getUsageLogGroupSummary(other?.group_ratio, other?.user_group_ratio);
    return { segments: [
      groupText ? { text: groupText, tone: 'primary' } : null,
      { text: t('dashboard.violationFee'), tone: 'primary' },
      { text: t('dashboard.violationFeeDetail', { fee: renderQuota(feeQuota, 6) }), tone: 'secondary' },
      text ? { text: t('dashboard.detailText', { text }), tone: 'secondary' } : null,
    ].filter(Boolean) };
  }
  return { segments: other?.claude
    ? renderModelPriceSimple(other.model_ratio, other.model_price, other.group_ratio, other?.user_group_ratio, other.cache_tokens || 0, other.cache_ratio || 1.0, other.cache_creation_tokens || 0, other.cache_creation_ratio || 1.0, other.cache_creation_tokens_5m || 0, other.cache_creation_ratio_5m || other.cache_creation_ratio || 1.0, other.cache_creation_tokens_1h || 0, other.cache_creation_ratio_1h || other.cache_creation_ratio || 1.0, false, 1.0, other?.is_system_prompt_overwritten, 'claude', getInitialBillingDisplayMode(), 'segments')
    : renderModelPriceSimple(other.model_ratio, other.model_price, other.group_ratio, other?.user_group_ratio, other.cache_tokens || 0, other.cache_ratio || 1.0, 0, 1.0, 0, 1.0, 0, 1.0, false, 1.0, other?.is_system_prompt_overwritten, 'openai', getInitialBillingDisplayMode(), 'segments'),
  };
}
function getUsageLogGroupSummary(groupRatio, userGroupRatio) {
  const parsedUserGroupRatio = Number(userGroupRatio);
  const useUserGroupRatio = Number.isFinite(parsedUserGroupRatio) && parsedUserGroupRatio !== -1;
  const ratio = useUserGroupRatio ? userGroupRatio : groupRatio;
  if (ratio === undefined || ratio === null || ratio === '') return '';
  return `${useUserGroupRatio ? t('dashboard.dedicatedRatio') : t('dashboard.groupRatio')} ${formatRatio(ratio)}x`;
}
function formatRatio(ratio) { if (ratio === undefined || ratio === null) return '-'; if (typeof ratio === 'number') return ratio.toFixed(4); return String(ratio); }
const openChannelAffinityUsageCacheModal = (affinity) => { console.log('打开渠道缓存使用:', affinity) }
const getLogOther = (otherStr) => { if (!otherStr) return null; try { return JSON.parse(otherStr) } catch (e) { console.error('解析日志 other 失败', e); return null } }
const isModelMapped = (row) => { const o = getLogOther(row.other); return o?.is_model_mapped && o?.upstream_model_name }
const typeFormat = (value) => { if (value) return typeList.value.find((item) => item.value == value).label; return '-'; }
const renderGroup = (value) => value || t('dashboard.userGroup')
const timeFormat = (value) => value ? $TOOL.dateFormat(Number(String(value) + '000')) : '-'
const copyText = (text) => { try { navigator.clipboard.writeText(String(text || '')); ElMessage.success(t('dashboard.copied')) } catch (error) { console.error(error); } }
const queryTimeFormat = (value) => value ? new Date(value).getTime().toString().substring(0, 10) : ''
const getFirstTokenTagType = (ms) => { const s = (Number(ms) || 0) / 1000; if (s < 3) return 'success'; if (s < 10) return 'warning'; return 'danger' }
const getTimeTagType = (t) => { const s = Number(t) || 0; if (s < 101) return 'success'; if (s < 300) return 'warning'; return 'danger' }
const formatToken = (val) => (Number(val) || 0).toLocaleString()
const getCacheSummary = (otherStr) => { const o = getLogOther(otherStr); const read = Number(o.cache_tokens) || 0; const write5m = Number(o.cache_creation_tokens_5m) || 0; const write1h = Number(o.cache_creation_tokens_1h) || 0; const write = write5m + write1h || Number(o.cache_creation_tokens) || 0; if (read <= 0 && write <= 0) return null; let res = []; if (read > 0) res.push(t('dashboard.cacheRead', { tokens: formatToken(read) })); if (write > 0) res.push(t('dashboard.cacheWrite', { tokens: formatToken(write) })); return res.join(' · ') }
const isSubscriptionBilled = (otherStr) => getLogOther(otherStr).billing_source === 'subscription'
const renderQuota = (quota, digits = 2) => { let quotaPerUnit = localStorage.getItem('newapi_quota_per_unit'); const quotaDisplayType = localStorage.getItem('newapi_quota_display_type') || 'USD'; quotaPerUnit = parseFloat(quotaPerUnit); if (quotaDisplayType === 'TOKENS') return renderNumber(quota); const resultUSD = quota / quotaPerUnit; let symbol = '$'; let value = resultUSD; if (quotaDisplayType === 'CNY') { const statusStr = localStorage.getItem('newapi_status'); let usdRate = 1; try { if (statusStr) { const s = JSON.parse(statusStr); usdRate = s?.usd_exchange_rate || 1; } } catch (e) { } value = resultUSD * usdRate; symbol = '¥'; } else if (quotaDisplayType === 'CUSTOM') { const statusStr = localStorage.getItem('newapi_status'); let symbolCustom = '¤'; let rate = 1; try { if (statusStr) { const s = JSON.parse(statusStr); symbolCustom = s?.custom_currency_symbol || symbolCustom; rate = s?.custom_currency_exchange_rate || rate; } } catch (e) { } value = resultUSD * rate; symbol = symbolCustom; } const fixedResult = value.toFixed(digits); if (parseFloat(fixedResult) === 0 && quota > 0 && value > 0) { const minValue = Math.pow(10, -digits); return symbol + minValue.toFixed(digits); } return symbol + fixedResult; }
const renderNumber = (num) => { if (num >= 1000000000) return (num / 1000000000).toFixed(1) + 'B'; if (num >= 1000000) return (num / 1000000).toFixed(1) + 'M'; if (num >= 10000) return (num / 1000).toFixed(1) + 'k'; return num; }
const getChannelPath = (row) => { const info = getLogOther(row.other)?.admin_info; if (info?.use_channel?.length) return t('dashboard.channelPath', { path: info.use_channel.join('->') }); return t('dashboard.channelPath', { path: row.channel }); }
const numFormat = (value) => value ? (value * 0.000002).toFixed(6) : '0.00'

const inquires = (e) => {
  let obj = { ...e };
  if (e.dateTime) { obj.start_timestamp = queryTimeFormat(e.dateTime[0]); obj.end_timestamp = queryTimeFormat(e.dateTime[1]); delete obj.dateTime; }
  queryInfo.value = { ...obj }; getData(); getStatData();
};

const reset = () => {
  queryInfo.value = {}; getData(); getStatData();
  nextTick(() => { if (jmisSearchRef.value) jmisSearchRef.value.setData({ type: '0', dateTime: [$TOOL.dateFormat(new Date(new Date().setHours(0, 0, 0, 0))), $TOOL.dateFormat(new Date())] }); })
};

const handleSuccess = () => { getData(); };
const submissionData = () => {};
const openColSetting = () => { jmisTableRef.value.openColumnSetting(); };
const selectionChangeHandle = (sel) => { selected.value = sel; };

const buttonClick = (e) => {
  if (e == t("dashboard.btnView")) table_show(selected.value);
  else if (e == t("dashboard.btnDelete")) remove(selected.value);
  else if (e == t("dashboard.btnExportSelected")) exportExcel();
  else if (e == t("dashboard.btnClearSelection")) jmisTableRef.value.clearSelection();
  else if (e == t("dashboard.btnExportAll")) exportExcel();
};

const table_show = (row) => {
  dialog.save = true;
  nextTick(() => {
    if (Array.isArray(row) && row.length == 1) saveDialogRef.value.open("show").setData(JSON.parse(JSON.stringify(row[0])));
    else ElMessage({ message: t('dashboard.selectOneItem'), type: "warning" });
  });
};

const remove = (e) => {
  if (e.length == 0) { ElMessage({ message: t('dashboard.selectDataToDelete'), type: "warning" }); return; }
  let ids = e.map(el => el.id);
  apiObj.remove(ids).then((res) => { if (res.code === 200) { ElMessage({ message: t('dashboard.deleteSuccess'), type: "success" }); jmisTableRef.value.refresh(); } }).catch(err => console.error(err));
};

const exportExcel = () => {
  let ids = selected.value.map(e => e.id);
  if (ids.length <= 0) { ElMessage({ message: t('dashboard.selectDataToExport'), type: "warning" }); return; }
  apiObj.exportExcel(ids).then((res) => {
    const url = URL.createObjectURL(new Blob([res.data]));
    const link = document.createElement("a"); link.style.display = "none"; link.href = url;
    let filename = res.headers["content-disposition"].substr(20); filename = decodeURI(filename);
    link.download = filename; document.body.appendChild(link); link.click(); document.body.removeChild(link); URL.revokeObjectURL(url);
  }).catch(err => console.error(err));
};

onMounted(() => {
  getStatData();
  nextTick(() => { if (jmisSearchRef.value) jmisSearchRef.value.setData({ type: '0', dateTime: [$TOOL.dateFormat(new Date(new Date().setHours(0, 0, 0, 0))), $TOOL.dateFormat(new Date())] }); })
});
</script>

<style lang="scss" scoped>
.custom-header { padding-top: 15px; padding-bottom: 15px; border-bottom: 1px solid var(--border-color, #e6e6e6); display: flex; justify-content: space-between; }
.right-panel { display: flex; align-items: center; gap: 10px; }
.right-panel .el-tag { --el-tag-font-size: var(--font-size-base); }
.row-content { line-height: 18px; overflow: hidden; text-overflow: ellipsis; display: -webkit-box; -webkit-box-orient: vertical; -webkit-line-clamp: 2; }

/* 实心按钮 & 分页跟随主题色 */
:deep(.el-button:not(.is-plain)) {
  background: var(--btn-primary-hover-bg) !important;
  color: var(--btn-primary-text) !important;
  border-color: var(--btn-primary-border) !important;
  &:hover {
    background: var(--btn-primary-bg) !important;
    color: var(--btn-primary-hover-text) !important;
    border-color: var(--btn-primary-hover-border) !important;
  }
}
/* plain 按钮（如重置）边框和文字跟随主题 */
:deep(.el-button.is-plain) {
  // color: var(--btn-primary-bg) !important;
  // border-color: var(--btn-primary-bg) !important;
  // background: transparent !important;
  // &:hover {
  //   color: var(--btn-primary-hover-text) !important;
  //   background: var(--btn-primary-hover-bg) !important;
  //   border-color: var(--btn-primary-hover-border) !important;
  // }
  background: var(--btn-primary-hover-bg) !important;
  color: var(--btn-primary-text) !important;
  border-color: var(--btn-primary-border) !important;
  &:hover {
    background: var(--btn-primary-bg) !important;
    color: var(--btn-primary-hover-text) !important;
    border-color: var(--btn-primary-hover-border) !important;
  }
}
:deep(.el-pagination .el-pager li.is-active) {
  background: var(--btn-primary-bg) !important;
  color: var(--btn-primary-text) !important;
}
:deep(.el-pagination .el-pager li:not(.is-active):hover) {
  color: var(--btn-primary-bg) !important;
}

/* 搜索区域下拉框与输入框高度一致 */
:deep(.el-form .el-select__wrapper),
:deep(.jmis-search .el-select__wrapper) {
  height: 32px !important;
  min-height: 32px !important;
  box-sizing: border-box !important;
}
</style>

<style>
/* 调用看板输入框独立样式：不受全局输入框样式影响 */
.el-container[data-theme-id] .el-input__wrapper,
.el-container[data-theme-id] .el-select__wrapper {
  background: transparent !important;
  border: 1px solid var(--border-color) !important;
  min-height: unset !important;
  height: unset !important;
  box-shadow: none !important;
}
.el-container[data-theme-id] .el-input__inner,
.el-container[data-theme-id] .el-select__placeholder { color: var(--text-primary) !important; }
.el-container[data-theme-id] .el-input__inner { background: transparent !important; }

/* === 暗色主题 === */
.dark .el-input__wrapper,
.dark .el-select__wrapper { background: transparent !important; box-shadow: 0 0 0 1px rgba(255,255,255,0.20) !important; }
.dark .el-input__inner { color: #fff !important; background: transparent; }
.dark .el-select-dropdown { background: rgba(255,255,255,0.10) !important; border-color: rgba(255,255,255,0.20) !important; }
.dark .el-select-dropdown__item { color: #fff !important; }
.dark .el-select-dropdown__item.hover,
.dark .el-select-dropdown__item:hover { background: rgba(255,255,255,0.08) !important; }
.dark .el-table { --el-table-bg-color: rgba(255,255,255,0.05) !important; --el-table-tr-bg-color: rgba(255,255,255,0.05) !important; --el-table-header-bg-color: rgba(255,255,255,0.08) !important; --el-table-text-color: #fff !important; --el-table-header-text-color: #fff !important; --el-table-border-color: rgba(255,255,255,0.08) !important; }
.dark .el-pagination__total,
.dark .el-pagination__jump,
.dark .el-pager li { color: #fff !important; }
.dark .el-tag,
.dark .el-descriptions__title,
.dark .el-descriptions__label,
.dark .el-descriptions__content { color: #fff !important; }

/* === 情景主题（除雾灰调）=== */
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .el-container,
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .custom-header,
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .el-tag,
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .el-descriptions__title,
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .el-descriptions__label,
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .el-descriptions__content,
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .el-form-item__label { color: #fff !important; }
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .el-input__wrapper,
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .el-select__wrapper { background: transparent !important; box-shadow: 0 0 0 1px rgba(255,255,255,0.50) !important; }
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .el-input__inner { color: #fff !important; }
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .el-select__caret { color: #fff !important; }
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .el-range-input,
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .el-range-separator { color: #fff !important; }
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .right-panel .el-tag { background: rgba(255,255,255,0.10) !important; }
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .el-table__empty-text,
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .el-empty__description { --el-text-color-secondary: #fff !important; color: #fff !important; }
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .el-empty__description svg { color: #fff !important; }
/* 下拉框弹出部分（teleported 到 body，需要从 html 级别匹配） */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .el-select-dropdown { --el-bg-color-overlay: rgba(255,255,255,0.20) !important; background: rgba(255,255,255,0.20) !important; border-color: rgba(255,255,255,0.50) !important; }
/* 情景主题下拉背景偏暗，选项文字应为浅色（与分组标题一致），不能用黑字 */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .el-select-dropdown__item { --el-text-color-regular: #fff !important; color: #fff !important; }
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .el-select-dropdown__item.hover,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .el-select-dropdown__item:hover { background: rgba(255,255,255,0.08) !important; color: #fff !important; }
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .el-select-dropdown__item.selected { color: #fff !important; }
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .el-table { --el-table-bg-color: rgba(255,255,255,0.05) !important; --el-table-tr-bg-color: rgba(255,255,255,0.05) !important; --el-table-header-bg-color: rgba(255,255,255,0.08) !important; --el-table-text-color: #fff !important; --el-table-header-text-color: #fff !important; --el-table-border-color: rgba(255,255,255,0.08) !important; }
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .el-pagination__total,
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .el-pagination__jump,
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .el-pager li { color: #fff !important; }

/* === 雾灰调 === */
.light [data-theme-id="scene-graphite-cyan"] .el-input__wrapper,
.light [data-theme-id="scene-graphite-cyan"] .el-select__wrapper { background: rgba(0,0,0,0.05) !important; box-shadow: 0 0 0 1px rgba(0,0,0,0.10) !important; }
.light [data-theme-id="scene-graphite-cyan"] .el-table { --el-table-bg-color: rgba(0,0,0,0.03) !important; --el-table-tr-bg-color: rgba(0,0,0,0.03) !important; --el-table-header-bg-color: rgba(0,0,0,0.05) !important; }
</style>
