// Minimal config stub for jmisTable column settings
// The full version would persist column layout preferences per user

export default {
  request: {
    page: 'p',
    pageSize: 'page_size',
    sort: 'sort',
    direction: 'direction',
  },
  successCode: 200,

  parseData(res: any) {
    return res
  },

  async columnSettingGet(_tableName: string, column: any[]) {
    return {
      columnList: column,
      orderByList: [],
    }
  },

  async columnSettingSave(
    _tableName: string,
    _type: string,
    _userCol: any[],
    _orderList: any[],
  ) {
    return true
  },

  async columnSettingReset(_tableName: string, column: any[]) {
    return column
  },
}
