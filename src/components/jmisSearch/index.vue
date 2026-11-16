<template>
  <el-header ref="search" style="
      padding-bottom: 0;
      position: relative;
      border-bottom: 1px solid #e6e6e6;
    " :height="height">
    <el-form :inline="true" :model="queryform" style="height: auto" @submit.prevent>
      <div v-for="(item, index) in searchItem" :key="index" ref="formItem" style="display: inline-block">
        <el-form-item :label="item.label" v-if="item.type === 'switch'">
          <el-switch v-model="queryform[item.prop]" />
        </el-form-item>

        <el-form-item :label="item.label" v-if="item.type === 'input' && !item.clickInput">
          <el-input v-model="queryform[item.prop]" @keydown.enter.stop="searchFn" />
        </el-form-item>

        <el-form-item :label="item.label" v-if="item.type === 'input' && item.clickInput">
          <el-input v-model="queryform[item.prop]" @keydown.enter.stop="searchFn" @click="openinput(item.clickInput)" />
        </el-form-item>

        <el-form-item :label="item.label" v-if="item.type === 'time'">
          <el-date-picker v-model="queryform[item.prop]" type="daterange" :start-placeholder="$t('common.startDate')"
            :end-placeholder="$t('common.endDate')" :default-time="item.defaultTime" format="YYYY-MM-DD" value-format="YYYY-MM-DD"
            clearable />
        </el-form-item>

        <el-form-item :label="item.label" v-if="item.type === 'datetimerange'">
          <el-date-picker v-model="queryform[item.prop]" type="datetimerange" :start-placeholder="$t('common.startTime')"
            :end-placeholder="$t('common.endTime')" :default-time="item.defaultTime" format="YYYY-MM-DD HH:mm:ss" value-format="YYYY-MM-DD HH:mm:ss"
            clearable />
        </el-form-item>

        <el-form-item :label="item.label" v-if="item.type === 'check'" class="search-check">
          <el-checkbox v-model="queryform[item.prop]" :true-label="item.trueLabel" :false-label="item.falseLabel" />
        </el-form-item>

        <el-form-item :label="item.label" v-if="item.type === 'month'">
          <el-date-picker v-model="queryform[item.prop]" type="month" format="YYYY-MM" value-format="YYYY-MM"
            :placeholder="$t('common.selectMonth')" clearable />
        </el-form-item>

        <el-form-item :label="item.label" v-if="item.type === 'yeartime'">
          <el-date-picker v-model="queryform[item.prop]" type="year" format="YYYY[年]" value-format="YYYY"
            :placeholder="$t('common.selectYear')" clearable />
        </el-form-item>

        <el-form-item :label="item.label" v-if="item.type === 'date'">
          <el-date-picker v-model="queryform[item.prop]" type="date" value-format="YYYY-MM-DD" clearable />
        </el-form-item>

        <el-form-item :label="item.label" v-if="item.type === 'months'">
          <el-date-picker v-model="queryform[item.prop]" type="month" format="M[月]" value-format="M" :placeholder="$t('common.selectMonth')"
            clearable />
        </el-form-item>

        <el-form-item :label="item.label" v-if="item.type === 'week'">
          <el-date-picker v-model="queryform[item.prop]" type="week" format="w[周]" :placeholder="$t('common.selectWeek')" clearable />
        </el-form-item>

        <el-form-item :label="item.label" v-if="item.type === 'select'">
          <el-select v-model="queryform[item.prop]" clearable filterable style="width: 197px;">
            <el-option v-for="(selectItem, selectIndex) in item.selectItem" :key="selectIndex"
              :label="selectItem[item.labelText]" :value="selectItem[item.valueText]" />
          </el-select>
        </el-form-item>
      </div>

      <div ref="slotItem" style="display: inline-block">
        <slot></slot>
      </div>

      <div ref="btnWarp" style="display: inline-block">
        <el-form-item>
          <el-button :icon="Search" @click="searchFn" color="#626aef">{{ $t('common.query') }}</el-button>
          <el-button :icon="Refresh" @click="reset" color="#626aef" plain>{{ $t('common.reset') }}</el-button>
          <slot name="button"></slot>
        </el-form-item>
      </div>
    </el-form>
  </el-header>
</template>
<script setup>
import { ref, reactive, computed, watch, onMounted, nextTick } from "vue";
import { useI18n } from 'vue-i18n';
import { debounce } from "lodash-es";
import {
  Search,
  Refresh,
} from "@element-plus/icons-vue";
import {
  ElForm,
  ElFormItem,
  ElButton,
  ElInput,
  ElSwitch,
  ElSelect,
  ElOption,
  ElDatePicker,
  ElCheckbox,
  ElHeader,
} from "element-plus";

const { t } = useI18n()

const props = defineProps({
  searchItem: {
    type: Array,
    default: () => [
      {
        label: "综合查询",
        prop: "FuzzyWord",
        type: "input",
      },
    ],
  },
  height: {
    type: String,
    default: "auto",
  },
});

const emit = defineEmits(["inputclick", "search", "reset"]);

const queryform = reactive({});
const searchFlag = ref(false);
const noneEl = ref([]);
const formItemWidthList = ref([]);

const search = ref(null);
const btnWarp = ref(null);
const formItem = ref([]);
const slotItem = ref([]);

const openinput = (item) => {
  emit("inputclick", item);
};

const unwatch = () => {
  watch(
    () => props.searchItem,
    (val) => {
      for (const item of val) {
        if (item.default !== undefined) {
          queryform[item.prop] = item.default;
          searchFn();
        }
      }
    },
    { deep: true, immediate: true }
  );
};

const starchFlag = () => {
  const list = [...formItem.value, slotItem.value];
  list.forEach((e) => {
    if (e) e.style.display = "inline-block";
  });
  noneEl.value = [];
  searchFlag.value = false;
};

const getFormItemWidth = () => {
  const arr = [];
  const list = [...formItem.value, slotItem.value];
  list.forEach((e) => {
    arr.push(e.offsetWidth);
  });
  formItemWidthList.value = arr;
};

const searchFn = () => {
  for (const key in queryform) {
    if (!queryform[key] && queryform[key] !== 0) {
      delete queryform[key];
    }
  }
  emit("search", queryform);
};

const reset = () => {
  Object.keys(queryform).forEach((key) => delete queryform[key]);
  props.searchItem.forEach((item) => {
    if (item.type === "check") {
      queryform[item.prop] = item.falseLabel;
    }
  });
  emit("reset", queryform);
};

const setData = (data) => {
  Object.keys(data).forEach((key) => {
    queryform[key] = data[key];
  })
}

onMounted(() => {
  nextTick(() => {
    unwatch();
    getFormItemWidth();
    starchFlag();
    window.addEventListener("resize", debounce(starchFlag, 120));
  });
});

defineExpose({
  setData,
})
</script>

<style lang="scss" scoped>
:deep(.el-form-item) {
  margin-bottom: 13px;
}

.search-check {
  margin-right: 18px !important;

  :deep(.el-form-item__label) {
    padding-right: 7px !important;
  }
}
</style>