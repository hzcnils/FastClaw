<template>
  <el-dialog :title="titleMap[mode]" top="5vh" v-model="visible" width="55%" destroy-on-close @closed="$emit('closed')"
    :close-on-press-escape="false" :close-on-click-modal="false">
    <el-form :model="form" :rules="rules" :disabled="mode === 'show'" ref="dialogForm" label-width="auto">
      <el-collapse v-model="activeNames" @change="handleChange">
        <el-collapse-item name="1">
          <template #title>
            <div class="header-title">
              <i class="header-num">1</i>
              基本信息
              <el-tooltip class="box-item" effect="dark" content="设置令牌的基本信息" placement="top">
                <el-button class="header-icon" type="text" :icon="InfoFilled"></el-button>
              </el-tooltip>
            </div>
          </template>
          <el-row :gutter="24">
            <el-col :span="12">
              <el-form-item label="名称" prop="title">
                <el-input v-model="form.title" :placeholder="'请输入'" clearable></el-input>
              </el-form-item>
            </el-col>
            <el-col :span="12">
              <el-form-item label="过期时间" prop="telegraphNumber">
                <el-date-picker v-model="form.telegraphNumber" type="datetime" placeholder="请输入" :shortcuts="shortcuts" style="width: 100%" />
              </el-form-item>
            </el-col>
          </el-row>
        </el-collapse-item>
        <el-collapse-item name="2">
          <template #title>
            <div class="header-title">
              <i class="header-num">2</i>额度设置
              <el-tooltip class="box-item" effect="dark" content="设置令牌可用额度和数量" placement="top">
                <el-button class="header-icon" type="text" :icon="InfoFilled"></el-button>
              </el-tooltip>
            </div>
          </template>
          <el-row :gutter="24">
            <el-col :span="12">
              <el-form-item label="额度" prop="orgId">
                <el-input v-model="form.orgId" :placeholder="'请输入'" clearable></el-input>
              </el-form-item>
            </el-col>
            <el-col :span="12">
              <el-form-item label="新建数量" prop="docculateType">
                <el-input v-model="form.docculateType" :placeholder="'请输入'" clearable></el-input>
              </el-form-item>
            </el-col>
          </el-row>
        </el-collapse-item>
        <el-collapse-item name="3">
          <template #title>
            <div class="header-title">
              <i class="header-num">3</i>访问设置
              <el-tooltip class="box-item" effect="dark" content="设置令牌可用访问和数量" placement="top">
                <el-button class="header-icon" type="text" :icon="InfoFilled"></el-button>
              </el-tooltip>
            </div>
          </template>
          <el-row :gutter="24">
            <el-col :span="12">
              <el-form-item label="IP白名单" prop="mj">
                <el-input v-model="form.title" :placeholder="'请输入'" clearable></el-input>
              </el-form-item>
            </el-col>
            <el-col :span="12">
              <el-form-item label="模型限制" prop="draftPerson">
                <el-input v-model="form.title" :placeholder="'请输入'" clearable></el-input>
              </el-form-item>
            </el-col>
          </el-row>
        </el-collapse-item>
        <el-collapse-item name="4">
          <template #title>
            <div class="header-title">
              <i class="header-num">4</i> 分组信息
              <el-tooltip class="box-item" effect="dark" content="设置令牌的分组" placement="top">
                <el-button class="header-icon" type="text" :icon="InfoFilled"></el-button>
              </el-tooltip>
            </div>
          </template>
          <el-row :gutter="24">
            <el-col :span="12">
              <el-form-item label="令牌分组" prop="createDate">
                <el-input v-model="form.title" :placeholder="'请输入'" clearable></el-input>
              </el-form-item>
            </el-col>
          </el-row>
        </el-collapse-item>
      </el-collapse>
    </el-form>

    <template #footer>
      <span class="dialog-footer">
        <el-button @click="visible = false">取 消</el-button>
        <el-button type="primary" :loading="isSaveing" @click="submit">确 定</el-button>
      </span>
    </template>
  </el-dialog>
</template>

<script setup>
import { ref, reactive } from "vue";
import {
  ElFormItem, ElCol, ElRow, ElDatePicker, ElInput, ElButton,
  ElMessage, ElAlert, ElForm, ElCollapse, ElCollapseItem, ElTooltip, ElDialog,
} from "element-plus";
import { InfoFilled } from "@element-plus/icons-vue";

const emit = defineEmits(["success", "closed"]);
const titleMap = { add: "新增", edit: "编辑", show: "查看" };
const mode = ref("add");
const visible = ref(false);
const isSaveing = ref(false);
const activeNames = ref(["1", "2", "3", "4"]);
const form = reactive({});
const rules = reactive({
  title: [{ required: true, message: "请输入", trigger: "blur" }],
  telegraphNumber: [{ required: true, message: "请输入", trigger: "blur" }],
  orgId: [{ required: true, message: "请输入", trigger: "blur" }],
});

const dialogForm = ref(null);

const open = (m = "add") => {
  mode.value = m;
  visible.value = true;
};

const submit = async () => {
  if (!dialogForm.value) return;
  const valid = await dialogForm.value.validate((valid) => valid);
  if (valid) {
    isSaveing.value = true;
    try {
      isSaveing.value = false;
      emit("success", { ...form }, mode.value);
      visible.value = false;
      ElMessage.success("操作成功");
    } catch (e) {
      isSaveing.value = false;
      ElMessage.warning("保存失败，请稍后重试");
    }
  } else {
    isSaveing.value = false;
    return false;
  }
};

const setData = (data) => {
  Object.assign(form, data);
};

const shortcuts = [
  { text: "永久", value: new Date() },
  { text: "一小时", value: () => { const d = new Date(); d.setTime(d.getTime() - 3600 * 1000 * 24); return d; } },
  { text: "一天", value: () => { const d = new Date(); d.setTime(d.getTime() - 3600 * 1000 * 24 * 7); return d; } },
  { text: "一个月", value: () => { const d = new Date(); d.setTime(d.getTime() - 3600 * 1000 * 24 * 7); return d; } },
];

defineExpose({ open, setData });
</script>

<style lang="scss" scoped>
.header-title {
  display: flex;
  align-items: center;
  .header-num {
    display: inline-block;
    margin: 0 16px 0 8px;
    width: 26px;
    height: 26px;
    line-height: 26px;
    color: #fff;
    font-size: var(--font-size-lg);
    text-align: center;
    font-style: normal;
    background-color: #25a6f7;
    border-radius: 50%;
  }
  .header-icon { font-size: var(--font-size-lg); margin-left: 10px; color: #5d5d5d; }
}
:deep(.el-collapse-item__header) { background: #f5f5f5; margin-bottom: 20px; }
</style>
