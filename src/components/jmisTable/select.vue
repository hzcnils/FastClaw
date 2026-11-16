<template>
  <el-dialog v-model="visible" title="" width="60%" :close-on-click-modal="false" destroy-on-close
    @closed="$emit('closed')">
    <el-container>
      <el-header>
        <div class="left-panel">
          <jmisButton :listItem="listItem" @buttonClick="buttonClick" />
        </div>
      </el-header>
      <el-main style="height: 650px">
        <jmisTable ref="projectTable" :data="tableDataList" :column="column" :params="peopleInfo"
          @selectionChange="selectionUserChangeHandle" :hidePagination="true">
          <el-table-column :reserve-selection="true" type="selection" align="center" width="50" />
          <template #pagination>
            <label @click="selectSize" style="font-size: 14px; color: #000000bf; margin-right: 10px" v-if="seleted">已选中 {{
              size }} 条</label>
            <el-pagination @size-change="handleSizeChange" @current-change="handleCurrentChange"
              :current-page="tableParams.current" :page-sizes="[10, 20, 50, 100, 200, 500, 1000]" background :small="true"
              layout="total, sizes, prev, pager, next, jumper" :total="total"
              :page-size="tableParams.size"></el-pagination>
          </template>
        </jmisTable>
      </el-main>
    </el-container>
    <template #footer>
      <el-button @click="visible = false">取 消</el-button>
    </template>
  </el-dialog>
</template>

<script>
export default {
  data() {
    return {
      listItem: [
        {
          name: "生成制式表",
          type: "button",
          icon: "el-icon-edit",
          typeColor: "primary",
        },
        {
          name: "移除",
          type: "button",
          icon: "el-icon-delete",
          typeColor: "danger",
        },
      ],
      seletedList: [],
      seleted: false,
      visible: false,
      column: [],
      tableParams: {
        size: 10,
        current: 1,
      },
      tableData: [],
      tableDataList: [],
      total: 0,
      size: 0,
    };
  },
  methods: {
    buttonClick(e) {
      if (e == "生成制式表") {
      } else if (e == "移除") {
        if (this.seletedList.length) {
          for (var i of this.seletedList) {
            this.tableDataList = this.tableDataList.filter(row => { return row.id != i.id });
          }
          this.$refs.projectTable.clearSelection()
        } else {
          this.$message.warning("请勾选要移除的数据")
        }
      }
    },
    selectionUserChangeHandle(selection) {
      this.seleted = true;
      this.size = selection.length;
      this.seletedList = selection;
    },
    open(select, column) {
      this.tableData = select;
      this.column = column;
      this.$nextTick(() => {
        this.getData();
      });
      this.visible = true;
      return this;
    },
    getData() {
      this.total = this.tableData.length;
      var listStr = JSON.parse(JSON.stringify(this.tableData));
      this.tableDataList = listStr.splice(
        (this.tableParams.current - 1) * this.tableParams.size,
        this.tableParams.size
      );
    },
    // 切换条数
    handleSizeChange(e) {
      this.tableParams.size = e;
      this.tableParams.current = 1;
      this.getData();
    },
    // 切换分页
    handleCurrentChange(e) {
      this.tableParams.current = e;
      this.getData();
    },
  },
};
</script>

<style></style>