<!--
 * @Descripttion: 数据表格组件
 * @version: 1.3
 * @Author: sakuya
 * @Date: 2021年10月19日16:04:54
 * @LastEditors:
 * @LastEditTime:
-->
<template>
  <div class="jmisTable" :style="{ height: _height, width: '100%' }" ref="scTableMain" v-loading="loading">
    <div class="jmisTable-table">
      <el-table border v-bind="$attrs" :data="tableData" :row-key="(row) => row.id" :key="toggleIndex"
        :highlight-current-row="highlightcurrentrow" ref="jmisTableRef" height="100%" tooltip-effect="none"
        @expand-change="expandChange" @sort-change="sortChange" @filter-change="filterChange" @cell-click="handClick"
        @current-change="highlightcurrentrowChange" @row-dblclick="dbClick" @selection-change="changeselect"
        :tree-props="{ children: 'children', hasChildren: 'hasChildren' }" :row-class-name="rowclassname"
        :cell-class-name="cellClassName" :show-summary="summary" :stripe="stripe">
        <slot></slot>

        <template v-for="(item, index) in userColumn" :key="index">
          <el-table-column v-if="!item.hide" :column-key="item.prop" :show-overflow-tooltip="true" :label="item.label"
            :prop="item.prop" :width="item.width" :align="item.align" :sortable="item.sortable ? 'custom' : false"
            :fixed="item.fixed" :filters="item.filters" :filter-method="remoteFilter || !item.filters ? null : filterHandler
              " :sort-by="sortByList">
            <template #default="scope">
              <slot v-if="item.imgShow" :name="item.prop" v-bind="{ scope }">
                <div v-if="scope.row[item.prop]">
                  <el-button class="imgborder">
                    <el-image :src="scope.row[item.prop + '_url']" :preview-src-list="[scope.row[item.prop + '_url']]"
                      style="max-width: 80px; height: 40px" :onerror="errorGoodsImg" />
                  </el-button>
                </div>
              </slot>
              <slot v-else-if="item.isImg" :name="item.prop" v-bind="{ scope }">
                <el-image :src="scope.row[item.prop]" :preview-src-list="[scope.row[item.prop]]"
                  style="max-width: 80px; height: 40px" :onerror="errorGoodsImg" />
              </slot>
              <slot v-else-if="item.label === '图标'" :name="item.prop" v-bind="{ scope }">
                <div v-if="scope.row[item.prop]">
                  <i :class="scope.row[item.prop]"></i>{{ scope.row[item.prop] }}
                </div>
              </slot>
              <slot v-else-if="item.label === '是否高亮'" :name="item.prop" v-bind="{ scope }">
                <div v-if="scope.row[item.prop]">
                  <el-switch v-model="scope.row[item.prop]" disabled></el-switch>
                </div>
              </slot>
              <slot v-else :name="item.prop" v-bind="scope">
                <span v-if="scope.row[item.prop]" class="showOverTooltip">{{
                  scope.row[item.prop]
                }}</span>
              </slot>
            </template>
          </el-table-column>
        </template>
        <el-table-column width="1"></el-table-column>
        <template #empty>
          <el-empty :description="emptyText" :image-size="100">
            <el-button v-if="showEmptyError" type="text" @click="visibleErrorDetail = true">查看详情</el-button>
          </el-empty>
        </template>
      </el-table>
    </div>
    <div class="jmisTable-page" v-show="pageList">
      <div class="jmisTable-pagination">
        <label @click="selectSize" style="font-size: 14px; color: #000000bf; margin-right: 10px"
          :class="buttonclick == true ? 'active' : ''" v-if="seleted">已选中 {{ size }} 条</label>
        <el-pagination v-if="!hidePagination" @size-change="handleSizeChange" background size="small"
          :layout="paginationLayout" :total="totalNum" :page-sizes="pageSizes" :page-size="pageSize"
          v-model:current-page="currentPage" @current-change="paginationChange"></el-pagination>
        <slot name="pagination"></slot>
      </div>

      <div class="jmisTable-do" v-if="!hideDo">
        <el-badge v-if="size > 0" :value="size" class="item">
          <el-button @click="selectSize" circle :icon="ShoppingCartFull" style="margin-left: 15px">
          </el-button>
        </el-badge>
        <el-popover v-if="column" placement="bottom" title="" :width="580"
          v-model:visible="columnSettingVisible" trigger="manual" popper-class="col-setting-popper"
          :teleported="false" :show-arrow="false"
          @show="onShow" @after-leave="onLeave">
          <template #reference>
            <span ref="columnSettingVirtualRef" style="width:0;height:0;display:inline-block;overflow:hidden"></span>
          </template>
          <columnSetting v-if="customColumnShow" ref="columnSettingRef" @userChange="columnSettingChange"
            @priority="priorityFn" @save="columnSettingSave" @back="columnSettingBack" @close="closeColumnSetting"
            :column="userColumn"
            :tableName="tableName" :tableColumnsByBase="tableColumnsByBase" @tableWidthChange="tableWidthChange">
          </columnSetting>
        </el-popover>
      </div>
    </div>
    <el-dialog v-if="showEmptyError" v-model="visibleErrorDetail" title="异常详情" width="700px"
      @close="visibleErrorDetail = false">
      <el-descriptions :column="1">
        <el-descriptions-item label="状态码：">
          <el-tag type="danger" size="small">{{ emptyErrorData.code }}</el-tag>
        </el-descriptions-item>
        <el-descriptions-item label="数据：">{{
          emptyErrorData.data === null ? "Null" : error
        }}</el-descriptions-item>
        <el-descriptions-item label="异常内容：">{{
          emptyErrorData.msg
        }}</el-descriptions-item>
      </el-descriptions>
      <template #footer>
        <span class="dialog-footer">
          <el-button @click="visibleErrorDetail = false">关闭</el-button>
        </span>
      </template>
    </el-dialog>
    <el-dialog title="图片预览" v-model="dialogVisible" width="30%">
      <img :src="dialogTableVisible" style="display: block; margin: 0 auto; max-width: 100%" :onerror="errorGoodsImg" />
    </el-dialog>
    <el-dialog top="10vh" v-model="showCellText" title="" width="50%" @close="showCellText = false">
      <el-input disabled v-model="cellText" type="textarea" />
    </el-dialog>
    <selectListDialog v-if="selectvisbly" ref="selectList" @closed="selectvisbly = false" />
  </div>
</template>

<script setup>
import { ref, inject, computed, watch, onMounted, nextTick } from 'vue';
import { ElTable, ElTableColumn, ElPagination, ElButton, ElBadge, ElSwitch, ElInput,
  ElPopover, ElDialog, ElEmpty, ElDescriptions, ElImage, ElDescriptionsItem, ElTag
} from 'element-plus';
import { ShoppingCartFull } from '@element-plus/icons-vue'
import { useRoute } from 'vue-router';
import config from "@/config/table";
import columnSetting from "./columnSetting.vue";
import selectListDialog from "./select.vue";

const $API = inject("$API");
const $TOOL = inject("$TOOL");
const route = useRoute();
// 1. Props 定义
const props = defineProps({
  buttonclick: { type: Boolean, default: true },
  specialDate: { type: Boolean, default: false },
  pageList: { type: Boolean, default: true },
  size: { type: Number, default: 0 },
  tableName: { type: String, default: "" },
  apiObj: {
    type: Object,
    default: () => { },
  },
  apiFun: { type: String, default: "get" },
  params: { type: Object, default: () => ({}) },
  data: {
    type: Object,
    default: () => { },
  },
  height: { type: [String, Number], default: "100%" },
  parseData: { type: Function, default: null },
  rowKey: { type: Function, default: null },
  column: {
    type: Array,
    default: [],
  },
  stripe: { type: Boolean, default: false },
  remoteFilter: { type: Boolean, default: false },
  summary: { type: Boolean, default: false },
  remoteSort: { type: Boolean, default: false },
  hidePagination: { type: Boolean, default: false },
  hideDo: { type: Boolean, default: false },
  columnAdd: { type: Boolean, default: false },
  paginationLayout: {
    type: String,
    default: "total,sizes, prev, pager, next, jumper",
  },
  highlightcurrentrow: { type: Boolean, default: false },
  rowclassname: { type: Function, default: null },
  cellClassName: { type: Function, default: null },
  categoryId: { type: String, default: "" },
  total: {
    type: Number,
    default: 0,
  },
  pageOption: {
    type: Object,
    default: () => ({
      // pageSize: 10,
      // total: 0,
      // currentPage: 1,
    }),
  },
  isCompact: { type: Boolean, default: false },
  autoPageSize: { type: Boolean, default: true },
})

// 2. 事件定义
const emit = defineEmits([
  'cellclick', 'selectSize', 'selectChange', 'handleExpandChange', 'dbclick',
  'inputValue', 'pageChange', 'getSchedule', 'currentRow'
])

// 3. 响应式数据
const scTableMain = ref(null)
const jmisTableRef = ref(null)
const columnSettingRef = ref(null)
const selectList = ref(null)
const dialogVisible = ref(false)
const showCellText = ref(false)
const emptyText = ref("暂无数据")
const toggleIndex = ref(0)
const tableData = ref([])
const pageSizes = ref([5, 10, 20, 50, 100, 200, 500, 1000])
const pageSize = ref(10)
const totalNum = ref(0)
const currentPage = ref(1)
const prop = ref(null)
const order = ref(null)
const loading = ref(false)
const tableParams = ref({ ...props.params })
const userColumn = ref([])
const customColumnShow = ref(false)
const seleted = ref(false)
const dialogTableVisible = ref("")
const cloneColumn = ref([])
const prioritySwitch = ref(true)
const columnSettingVisible = ref(false)
const columnSettingVirtualRef = ref(null)
const sortByList = ref([])
const showEmptyError = ref(false)
const visibleErrorDetail = ref(false)
const emptyErrorData = ref({})
const tableColumnsByBase = ref([])
const cellText = ref("")
const selectvisbly = ref(false)
const selectionList = ref([])
const errorGoodsImg = ""

// 4. 计算属性
const _height = computed(() => {
  return Number(props.height) ? Number(props.height) + "px" : props.height
})

// 5. 方法
// 点击单元格触发
const handClick = (row, column, cell, event) => {
  emit("cellclick", row, column, cell, event)
}

// 点击已经选中条数
const selectSize = () => {
  if (props.isCompact) {
    emit("selectSize");
    return;
  }
  if (selectionList.value.length > 0 && props.column.length > 0) {
    selectvisbly.value = true;
    nextTick(() => {
      selectList.value.open(selectionList.value, props.column);
    });
  }
}

// 选择变化
const changeselect = (selection) => {
  selectionList.value = selection;
  emit("selectChange", selectionList.value);
}

// 获取行高
const getRowHeight = () => {
  return new Promise((resolve) => {
    const rowHeight = 40.8;
    setTimeout(() => {
      resolve(
        Math.round(
          Number(jmisTableRef.value.$refs.bodyWrapper.clientHeight) / rowHeight
        )
      );
    }, 100);
  });
}

// 优先级控制
const priorityFn = (e) => {
  prioritySwitch.value = !prioritySwitch.value;
  columnSettingVisible.value = e;
}

// 加载状态
const loadingfl = (loadingStatus) => {
  loading.value = loadingStatus;
}

// 展开行
const expandChange = (row, expandedRows) => {
  emit("handleExpandChange", row, expandedRows);
}

// 双击行
const dbClick = (e) => {
  emit("dbclick", e);
}

const rows = (row) => row.id;

// 驼峰转换
const toCamelCase = (str) => {
  return str.replace(/_([a-z])/g, (match, letter) => letter.toUpperCase());
}

// 获取自定义列
const getCustomColumn = async () => {
  let tableColumnSetting = [];
  let userColumnData = [];
  if (props.columnAdd == true) {
    const data = await $API.geoDataTool.gisEntity.getAllField({
      categoryId: props.categoryId,
    });
    let newData = data.data.map((item) => ({
      label: item.column_comment,
      prop: item.column_name,
    }));
    tableColumnSetting = newData.filter(
      (item) => !item.label.includes("id", "行政区域编码")
    );
    tableColumnSetting.sort((a, b) => {
      const isGeometryA = ["点", "线", "面"].includes(a.label);
      const isGeometryB = ["点", "线", "面"].includes(b.label);
      if (isGeometryA && !isGeometryB) return 1;
      if (!isGeometryA && isGeometryB) return -1;
      return 0;
    });
  } else {
    const data = await config.columnSettingGet(
      props.tableName,
      JSON.parse(JSON.stringify(props.column))
    );
    tableColumnSetting = data;
    if (tableColumnSetting.orderByList.length > 0) {
      userColumnData = tableColumnSetting.columnList.map((item) => {
        tableColumnSetting.orderByList.forEach((row) => {
          if (item.label === row.label) {
            item.priority = row.orderByPriority;
            item.asc = row.orderByType;
          }
        });
        return item;
      });
    } else {
      userColumnData = tableColumnSetting.columnList;
    }
  }
  if (tableColumnSetting.length > 0) {
    userColumnData = tableColumnSetting;
  }

  if (userColumnData.length > 0) {
    userColumn.value = userColumnData;
  } else {
    userColumn.value = JSON.parse(JSON.stringify(props.column));
  }
  if ($TOOL.data.get("SORT")) {
    let json = $TOOL.data.get("SORT");
    if (json.moduleId === route.name) {
      json.dataJson.forEach((item) => {
        for (let i = 0; i < userColumn.value.length; i++) {
          if (userColumn.value[i].label === item.label) {
            userColumn.value[i] = {
              ...item,
              ...userColumn.value[i],
            };
            break;
          }
        }
      });
    }
  }

  let data = [];
  for (let i = 1; i < 51; i++) {
    userColumn.value.forEach((item) => {
      if (item.priority === i) {
        data.push(item);
      }
    });
  }
  sortByList.value = [];
  data.forEach((item) => {
    sortByList.value.push(item.asc == "升序" ? item.prop : item.prop);
  });
}

// 获取表格数据
const getData = async (headerSort = false) => {
  loading.value = true;
  showEmptyError.value = false;
  emptyErrorData.value = {};
  emit("inputValue", pageSize.value, currentPage.value);

  var reqData = {
    [config.request.page]: currentPage.value,
    [config.request.pageSize]: pageSize.value,
    [config.request.sort]: headerSort ? prop.value ?? null : null,
    [config.request.direction]: headerSort ? order.value ?? null : null,
  };

  if (props.hidePagination) {
    delete reqData[config.request.page];
    delete reqData[config.request.pageSize];
  }

  Object.assign(reqData, tableParams.value);
  reqData[config.request.sort] = prop.value ? prop.value : tableParams.value.descs;

  try {
    var res = await props.apiObj[props.apiFun](reqData);
  } catch (error) {
    loading.value = false;
    emptyText.value = error.statusText;
    return false;
  }
  try {
    var response;
    if (props.parseData != null) {
      response = props.parseData(res);
    } else {
      response = config.parseData(res);
    }
  } catch (error) {
    loading.value = false;
    emptyText.value = "数据格式错误";
    return false;
  }
  if (response.code !== config.successCode) {
    loading.value = false;
    emptyText.value = "请求出现异常";
    showEmptyError.value = true;
    emptyErrorData.value = response;
  } else {
    emptyText.value = "暂无数据";
    tableData.value = response.rows || [];

    for (var v of userColumn.value) {
      if (v.imgShow) {
        tableData.value.forEach((e, i) => {
          setImgs(e[v.prop], i, v.prop);
        });
      }
    }
    totalNum.value = response.total || 0;
    loading.value = false;
  }
  setTimeout(() => {
    doLayout();
  }, 100);
  if (jmisTableRef.value && jmisTableRef.value.$el) {
    jmisTableRef.value.$el.querySelector(".el-table__body-wrapper").scrollTop = 0;
  }
  emit("getSchedule", tableData.value);
}

// 图片加载
const setImgs = (url, index, filed) => {
  if (url) {
    $API.model.common.file
      .downloadFile({ objectName: url })
      .then((res) => {
        const imgUrl = URL.createObjectURL(new Blob([res.data]));
        tableData.value[index][filed + "_url"] = imgUrl;
      });
  }
}

// 分页切换
const paginationChange = () => {
  getData(true);
  emit("pageChange", {
    pageSize: pageSize.value,
    total: totalNum.value,
    currentPage: currentPage.value,
  });
}

// 每页条数改变
const handleSizeChange = (val) => {
  pageSize.value = val;
  getData(true);
  emit("pageChange", {
    pageSize: pageSize.value,
    total: totalNum.value,
    currentPage: currentPage.value,
  });
}

// 刷新
const refresh = () => {
  jmisTableRef.value.clearSelection();
  getData();
}

// 更新数据
const upData = (params, page = 1) => {
  currentPage.value = page;
  jmisTableRef.value.clearSelection();
  tableParams.value = {};
  let paramss = Object.assign(tableParams.value, params || {});
  Object.keys(paramss).forEach((key) => {
    if (paramss[key] != null && paramss[key] != "") {
      tableParams.value[key] = paramss[key];
    }
  });
  getData();
}

// 重载数据
const reload = (params, page = 1) => {
  currentPage.value = page;
  tableParams.value = params || {};
  jmisTableRef.value.clearSelection();
  jmisTableRef.value.clearSort();
  jmisTableRef.value.clearFilter();
  getData();
}

// 列设置变化
const columnSettingChange = (col) => {
  userColumn.value = col;
  toggleIndex.value += 1;
}

// 保存列设置
const columnSettingSave = (userCol, type, orderList) => {
  columnSettingRef.value.isSave = true;
  config
    .columnSettingSave(props.tableName, type, userCol, orderList)
    .then(() => {
      getCustomColumn();
    });
  props.$message.success("保存成功");
  columnSettingRef.value.isSave = false;
}

// 重置列设置
const columnSettingBack = async () => {
  columnSettingRef.value.isSave = true;
  try {
    const column = await config.columnSettingReset(
      props.tableName,
      JSON.parse(JSON.stringify(props.column))
    );
    userColumn.value = column;
    columnSettingRef.value.usercolumn = JSON.parse(
      JSON.stringify(userColumn.value || [])
    );
  } catch (error) {
    props.$message.error("重置失败");
  }
  columnSettingRef.value.isSave = false;
}

// 排序
const sortChange = (obj) => {
  if (props.specialDate) {
    order.value = obj.order === "ascending" ? "asc" : obj.order === "descending" ? "desc" : null;
    const p = obj.prop;
    tableParams.value.sort = captitalize(p);
    tableParams.value.direction = order.value;
    delete tableParams.value.order;
    return getData(true);
  } else {
    order.value = obj.order === "ascending" ? "asc" : obj.order === "descending" ? "desc" : null;
    const p = obj.prop;
    if (p) {
      const arr = ["realAmount", "invoiceAmount"];
      if (arr.includes(p)) {
        prop.value = p;
        return getData(true);
      } else {
        if (props.tableName) {
          tableColumnsByBase.value.some((item) => {
            const column_name = item.column_name.replace("_", "");
            if (column_name === p.toLowerCase()) {
              prop.value = item.column_name;
              return true;
            }
          });
        } else {
          prop.value = captitalize(p);
          return getData(true);
        }
      }
    } else {
      prop.value = null;
    }
    getData(true);
  }
}

// 大写转下划线
const captitalize = (s) => {
  var words = s.split("");
  var reg = /^[A-Z]+$/;
  for (var i = 1; i < words.length; i++) {
    if (reg.test(words[i])) {
      words[i] = "_" + words[i];
    }
  }
  return words.join("");
}

// 本地过滤
const filterHandler = (value, row, column) => {
  const property = column.property;
  return row[property] === value;
}

// 过滤事件
const filterChange = (filters) => {
  if (!props.remoteFilter) return false;
  Object.keys(filters).forEach((key) => {
    filters[key] = filters[key].join(",");
  });
  upData(filters);
}

// 原生方法转发
const clearSelection = () => jmisTableRef.value.clearSelection()
const toggleRowSelection = (row, selected) => jmisTableRef.value.toggleRowSelection(row, selected)
const toggleAllSelection = () => jmisTableRef.value.toggleAllSelection()
const toggleRowExpansion = (row, expanded) => jmisTableRef.value.toggleRowExpansion(row, expanded)
const setCurrentRow = (row) => jmisTableRef.value.setCurrentRow(row)
const clearSort = () => jmisTableRef.value.clearSort()
const clearFilter = (columnKey) => jmisTableRef.value.clearFilter(columnKey)
const doLayout = () => jmisTableRef.value.doLayout()
const sort = (p, o) => jmisTableRef.value.sort(p, o)

// 当前行变化
const highlightcurrentrowChange = (val) => emit("currentRow", val)

// 计算宽度
const getTextWidth = (str) => {
  let padding = 0;
  let width = 0;
  let span = document.createElement("span");
  span.innerText = str;
  span.className = "getwidth";
  document.querySelector("body").appendChild(span);
  width = document.querySelector(".getwidth").offsetWidth + padding + 25;
  document.querySelector(".getwidth").remove();
  return width;
}

const getMaxLength = (arr, minwidth = 95) => {
  return arr.reduce((acc, item) => {
    if (item) {
      let calcLen = getTextWidth(item);
      if (acc < calcLen) acc = calcLen;
    }
    return acc < minwidth ? minwidth : acc;
  }, 0);
}

const calcColumnsWidth = (columns, tableArr) => {
  columns.forEach((item) => {
    const arr = tableArr.map((x) => x[item.prop]);
    item.width = getMaxLength(arr);
    arr.push(item.label);
  });
  return columns;
}

const tableWidthChange = (type) => {
  if (type == "自适应") {
    userColumn.value = calcColumnsWidth(userColumn.value, tableData.value);
  } else if (type == "初始化") {
    userColumn.value = [];
    nextTick(() => {
      userColumn.value = JSON.parse(JSON.stringify(cloneColumn.value));
      setTimeout(() => doLayout(), 50);
    });
  }
}

// 获取基础列
const getSuperQueryTableColumnsByBase = async () => {
  const res = await $API.system.policy.getSuperQueryTableColumnsByBase(
    { tableName: props.tableName }
  );
  tableColumnsByBase.value = res.data;
}

// 6. 监听器
watch(() => props.pageOption, (newval) => {
  currentPage.value = newval.current;
  pageSize.value = newval.size;
}, { deep: true })

watch(() => props.categoryId, async () => {
  await getCustomColumn();
  tableData.value = props.data;
}, { deep: true })

watch(() => props.data, async () => {
  cloneColumn.value = JSON.parse(JSON.stringify(props.column));
  if (props.columnAdd == false) await getCustomColumn();
  tableData.value = props.data;
  totalNum.value = props.total || tableData.value.length;
})

watch(() => props.apiObj, () => {
  tableParams.value = props.params;
  refresh();
})

watch(() => props.size, () => {
  seleted.value = true;
})

watch(userColumn, (val) => {
  if (!cloneColumn.value.length) {
    cloneColumn.value = JSON.parse(JSON.stringify(val));
  }
}, { deep: true })

// 7. 生命周期
onMounted(async () => {
  if (props.autoPageSize) {
    pageSize.value = await getRowHeight();
    if (!pageSizes.value.includes(pageSize.value)) {
      pageSizes.value.unshift(pageSize.value);
      pageSizes.value.sort((x, y) => x - y);
    }
  }
  if (props.column && props.columnAdd == false) await getCustomColumn();
  if (props.apiObj && Object.keys(props.apiObj).length > 0) {
    await getData();
  } else if (props.data) {
    tableData.value = props.data;
    totalNum.value = props.total || tableData.value.length;
  }
})

function openColumnSetting() {
  columnSettingVisible.value = true
}

function closeColumnSetting() {
  columnSettingVisible.value = false
}

function onShow() {
  customColumnShow.value = true
  setTimeout(() => {
    document.addEventListener('click', handleDocClick)
  }, 100)
}

function onLeave() {
  customColumnShow.value = false
  document.removeEventListener('click', handleDocClick)
}

function handleDocClick(e) {
  const popper = document.querySelector('.col-setting-popper')
  if (popper && !popper.contains(e.target)) {
    // 检查是否是 teleported 弹窗内部点击（el-dialog append-to-body / el-select-dropdown 等）
    const elOverlay = e.target.closest('.el-overlay')
    if (elOverlay) return
    const elPopper = e.target.closest('.el-popper')
    if (elPopper) return
    // 也检查 dropdown 的 popper（teleported 到 body 的下拉菜单）
    const dropdownPopper = document.querySelector('.el-popper.is-light.el-dropdown__popper')
    if (!dropdownPopper || !dropdownPopper.contains(e.target)) {
      columnSettingVisible.value = false
    }
  }
}

defineExpose({
  clearSelection, toggleRowSelection, toggleAllSelection, toggleRowExpansion,
  setCurrentRow, clearSort, clearFilter, doLayout, sort, reload, upData, refresh,
  openColumnSetting,
})
</script>

<style lang="scss" scoped>
.imgborder {
  border: none;
}

.jmisTable-table {
  height: calc(100% - 50px);
}

.jmisTable-page {
  height: 50px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 15px;
}

.jmisTable-do {
  white-space: nowrap;
}

:deep(.el-table .el-table__cell) {
  padding: 8px 0;
}

:deep(.el-table) {
  font-size: 12px;
}

:deep(.el-table .blue) {
  color: #07a8f5;
}

:deep(.el-table .purple) {
  color: #ba68c8;
}

:deep(.el-table .gray) {
  background-color: #f5f7fa !important;
}

:deep(.el-table__empty-block) {
  width: 100% !important;
  min-width: 100%;
  max-height: 100%;
  padding-right: 100px;
}

:deep(.el-empty__bottom) {
  margin-top: 0;
  line-height: 30px;
}

:deep(.el-empty__description) {
  margin-top: 30px;
  line-height: 30px;
}

:deep(.el-table__body-wrapper::-webkit-scrollbar) {
  width: 0px;
  height: 0px;
}

:deep(.el-table__body-wrapper:hover::-webkit-scrollbar) {
  width: 10px;
  height: 10px;
}

:deep(.el-textarea__inner) {
  min-height: 100px !important;
  margin-bottom: 20px;
}

.jmisTable-pagination {
  display: flex;
  align-items: center;
}

.active:hover {
  color: #07a8f5 !important;
}

.el-button :deep(span) {
  margin-left: 0px !important;
}

:deep(.el-tooltip__popper) {
  max-width: 100px !important;
}
</style>

<style lang="scss">
.col-setting-popper {
  position: fixed !important;
  top: 50% !important;
  left: 50% !important;
  transform: translate(-50%, -50%) !important;
  padding: 0 !important;
  background: #fff !important;
  border: 1px solid #dcdfe6 !important;
  border-radius: 8px !important;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.12) !important;
}

.col-setting-popper .el-popover__title {
  display: none;
}

.col-setting-popper .el-popover__content {
  background: inherit;
}

.col-setting-popper .setting-column {
  background: inherit;
}

.col-setting-popper .setting-column__bottom {
  background: inherit;
}

/* Dark theme */
.dark .col-setting-popper {
  background: #000 !important;
  border-color: rgba(255, 255, 255, 0.2) !important;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5) !important;
}
.dark .col-setting-popper .setting-column__header-title {
  color: #ffffff;
}
.dark .col-setting-popper .setting-column__header {
  border-bottom-color: rgba(255, 255, 255, 0.1);
}
.dark .col-setting-popper .setting-column__close-btn {
  color: rgba(255, 255, 255, 0.55);
}
.dark .col-setting-popper .setting-column__close-btn:hover {
  color: #ffffff;
  background: rgba(255, 255, 255, 0.08);
}
.dark .col-setting-popper .el-popover__title {
  color: #ffffff;
  border-bottom-color: rgba(255, 255, 255, 0.1);
}

/* Buttons follow system theme */
.col-setting-popper .el-button--primary {
  background: var(--btn-primary-bg) !important;
  color: var(--btn-primary-text) !important;
  border-color: var(--btn-primary-border) !important;
}
.col-setting-popper .el-button--primary:hover {
  background: var(--btn-primary-hover-bg) !important;
  color: var(--btn-primary-hover-text) !important;
}
.col-setting-popper .el-button--primary.is-plain {
  color: var(--btn-primary-bg) !important;
  border-color: var(--btn-primary-bg) !important;
  background: transparent !important;
}
.col-setting-popper .el-button--primary.is-plain:hover {
  color: var(--btn-primary-hover-text) !important;
  background: var(--btn-primary-hover-bg) !important;
}

/* Switches follow system theme */
.col-setting-popper .el-switch.is-checked .el-switch__core {
  background: var(--btn-primary-bg) !important;
  border-color: var(--btn-primary-bg) !important;
}

/* 排序优先级对话框 */
.col-priority-dialog {
  border-radius: 18px;
  box-shadow: 0 24px 70px rgba(15, 23, 42, 0.24);
}

/* === 情景主题（除雾灰调）：列设置 + 排序优先级 === */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .col-setting-popper {
  background: linear-gradient(135deg, rgba(255,255,255,0.06), transparent), var(--theme-dailog-color) !important;
  color: var(--text-primary, #fff) !important;
  border-color: var(--border-color, rgba(255,255,255,0.15)) !important;
}
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .col-setting-popper .setting-column__header-title,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .col-setting-popper .setting-column__title span,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .col-setting-popper .setting-column__list li > span,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .col-setting-popper .priority_title span { color: #fff !important; }
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .col-setting-popper .setting-column__header,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .col-setting-popper .setting-column__bottom,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .col-setting-popper .priority_title { border-color: rgba(255,255,255,0.15) !important; }
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .col-setting-popper .setting-column__close-btn { color: rgba(255,255,255,0.55) !important; }
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .col-setting-popper .setting-column__close-btn:hover { color: #fff !important; background: rgba(255,255,255,0.08) !important; }
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .col-setting-popper .el-input__wrapper { background: rgba(255,255,255,0.10) !important; box-shadow: 0 0 0 1px rgba(255,255,255,0.20) !important; }
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .col-setting-popper .el-input__inner { color: #fff !important; }
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .col-setting-popper .move { color: #fff !important; background: rgba(255,255,255,0.10) !important; border-color: transparent !important; }
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .col-setting-popper .move svg { color: #fff !important; }

/* 排序优先级对话框（append-to-body） */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .col-priority-dialog {
  background: linear-gradient(135deg, rgba(255,255,255,0.06), transparent), var(--theme-dailog-color) !important;
  color: var(--text-primary, #fff) !important;
  border-color: var(--border-color, rgba(255,255,255,0.15)) !important;
}
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .col-priority-dialog .el-dialog__title,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .col-priority-dialog .el-dialog__body,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .col-priority-dialog .el-dialog__body label,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .col-priority-dialog .priority_title span { color: #fff !important; }
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .col-priority-dialog .priority_table,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .col-priority-dialog .priority_title { border-color: rgba(255,255,255,0.15) !important; }
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .col-priority-dialog .el-input__wrapper { background: rgba(255,255,255,0.10) !important; box-shadow: 0 0 0 1px rgba(255,255,255,0.20) !important; }
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .col-priority-dialog .el-input__inner { color: #fff !important; }

/* === 暗色主题：列设置 + 排序优先级 === */
.dark .col-priority-dialog {
  background: linear-gradient(135deg, rgba(255,255,255,0.06), transparent), var(--theme-dailog-color) !important;
  color: #fff !important;
  border-color: rgba(255,255,255,0.15) !important;
}
.dark .col-priority-dialog .el-dialog__title,
.dark .col-priority-dialog .el-dialog__body,
.dark .col-priority-dialog .priority_title span { color: #fff !important; }
.dark .col-priority-dialog .priority_table,
.dark .col-priority-dialog .priority_title { border-color: rgba(255,255,255,0.12) !important; }
.dark .col-priority-dialog .el-input__wrapper { background: rgba(255,255,255,0.10) !important; box-shadow: 0 0 0 1px rgba(255,255,255,0.20) !important; }
.dark .col-priority-dialog .el-input__inner { color: #fff !important; }
.dark .col-priority-dialog .el-select .el-input__wrapper { background: rgba(255,255,255,0.10) !important; box-shadow: 0 0 0 1px rgba(255,255,255,0.20) !important; }

/* 空状态文字 */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .el-empty__description { --el-text-color-secondary: #fff !important; color: #fff !important; }
.dark .el-empty__description { --el-text-color-secondary: #fff !important; color: #fff !important; }
</style>
