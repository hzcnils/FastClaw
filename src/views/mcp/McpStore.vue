<script setup>
import { ref, computed, onMounted } from 'vue'
import { Plug, Download, Star, Search, Loader2, Package, Server } from 'lucide-vue-next'
import BackButton from '../common/BackButton.vue'
import { ElMessage } from 'element-plus'
import { mcpApi } from './adminApi.js'

const servers = ref([])
const loading = ref(false)
const searchQuery = ref('')
const installing = ref(new Set())

// 内置推荐 MCP Server 列表
const builtinServers = [
  {
    id: 'filesystem',
    name: 'filesystem',
    display_name: '文件系统',
    description: '读写本地文件和目录',
    author: 'Anthropic',
    downloads: 2500,
    rating: 4.9,
    install_type: 'npx',
    install_command: 'npx -y @modelcontextprotocol/server-filesystem',
  },
  {
    id: 'github',
    name: 'github',
    display_name: 'GitHub',
    description: '管理 GitHub 仓库、Issues、PR',
    author: 'Anthropic',
    downloads: 1800,
    rating: 4.8,
    install_type: 'npx',
    install_command: 'npx -y @modelcontextprotocol/server-github',
  },
  {
    id: 'postgres',
    name: 'postgres',
    display_name: 'PostgreSQL',
    description: '连接和查询 PostgreSQL 数据库',
    author: 'Anthropic',
    downloads: 1200,
    rating: 4.7,
    install_type: 'npx',
    install_command: 'npx -y @modelcontextprotocol/server-postgres',
  },
  {
    id: 'puppeteer',
    name: 'puppeteer',
    display_name: 'Puppeteer',
    description: '浏览器自动化和网页抓取',
    author: 'Anthropic',
    downloads: 950,
    rating: 4.6,
    install_type: 'npx',
    install_command: 'npx -y @modelcontextprotocol/server-puppeteer',
  },
  {
    id: 'slack',
    name: 'slack',
    display_name: 'Slack',
    description: '发送和接收 Slack 消息',
    author: 'Anthropic',
    downloads: 800,
    rating: 4.5,
    install_type: 'npx',
    install_command: 'npx -y @modelcontextprotocol/server-slack',
  },
  {
    id: 'sqlite',
    name: 'sqlite',
    display_name: 'SQLite',
    description: '查询 SQLite 数据库',
    author: 'Anthropic',
    downloads: 750,
    rating: 4.7,
    install_type: 'npx',
    install_command: 'npx -y @modelcontextprotocol/server-sqlite',
  },
]

const filteredServers = computed(() => {
  let list = builtinServers
  if (searchQuery.value.trim()) {
    const q = searchQuery.value.toLowerCase()
    list = list.filter(s =>
      s.name.toLowerCase().includes(q) ||
      s.display_name.toLowerCase().includes(q) ||
      s.description.toLowerCase().includes(q)
    )
  }
  return list
})

async function installServer(server) {
  if (installing.value.has(server.id)) return
  installing.value.add(server.id)

  try {
    // install_command 是整条命令行（"npx -y @scope/pkg"），FastPlan 的
    // transport_config 要求把命令和参数**分开**给：{"command","args"}。
    // 另外 transport_config 在 CreateMcpServerRequest 里是必填字段，
    // 少了它整个请求反序列化就失败（原来这里发的是顶层 command，装不上）。
    const [command, ...args] = String(server.install_command).split(/\s+/).filter(Boolean)
    await mcpApi.create({
      name: server.name,
      description: server.description,
      transport_type: 'stdio',
      transport_config: { command, args },
      auto_start: true,
    })
    ElMessage.success(`${server.display_name} 安装成功`)
  } catch (e) {
    ElMessage.error(`安装失败: ${e.message}`)
  } finally {
    installing.value.delete(server.id)
  }
}

onMounted(() => {
  // 可以从后端加载更多 MCP Server
})
</script>

<template>
  <div class="h-full flex flex-col bg-[#F5F7FA]">
    <div class="mx-auto w-full px-6 py-6 xl:px-20 2xl:px-[150px]" style="max-width: 1600px">
      <!-- Header -->
      <div class="flex items-start justify-between mb-8">
        <div class="flex items-start">
          <BackButton to="/" label="返回" class="mt-2" />
          <div>
            <div class="text-2xl font-bold text-[#1D2129] flex items-center gap-3">
              <div
                class="w-10 h-10 bg-gradient-to-br from-blue-500 to-indigo-600 rounded-xl flex items-center justify-center text-white shadow-lg shadow-blue-500/20">
                <Plug :size="20" />
              </div>
              MCP 商店
            </div>
            <div class="text-sm text-[#86909C] mt-2 ml-[52px]">发现和安装 MCP Server</div>
          </div>
        </div>
      </div>

      <!-- Search -->
      <div class="flex items-center gap-4 mb-6">
        <div class="relative flex-1 max-w-md">
          <Search :size="16" class="absolute left-3 top-1/2 -translate-y-1/2 text-gray-400" />
          <input v-model="searchQuery" placeholder="搜索 MCP Server..."
            class="w-full pl-9 pr-4 py-2.5 text-sm bg-white border border-gray-200 rounded-xl focus:border-blue-500 focus:ring-2 focus:ring-blue-500/10 outline-none" />
        </div>
      </div>

      <!-- Servers Grid -->
      <div v-if="filteredServers.length" class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
        <div v-for="server in filteredServers" :key="server.id"
          class="bg-white border border-gray-100 rounded-2xl p-5 shadow-sm hover:shadow-lg transition-all">
          <div class="flex items-start justify-between mb-3">
            <div class="flex items-center gap-3">
              <div class="w-10 h-10 bg-blue-50 rounded-xl flex items-center justify-center">
                <Server :size="20" class="text-blue-600" />
              </div>
              <div>
                <h3 class="text-sm font-semibold text-gray-800">{{ server.display_name }}</h3>
                <p class="text-xs text-gray-400">by {{ server.author }}</p>
              </div>
            </div>
          </div>
          <p class="text-xs text-gray-500 mb-4 line-clamp-2">{{ server.description }}</p>
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-3 text-xs text-gray-400">
              <span class="flex items-center gap-1">
                <Download :size="12" /> {{ server.downloads }}
              </span>
              <span class="flex items-center gap-1">
                <Star :size="12" class="fill-amber-400 text-amber-400" /> {{ server.rating }}
              </span>
            </div>
            <button @click="installServer(server)" :disabled="installing.has(server.id)"
              class="flex items-center gap-1.5 px-3 py-1.5 bg-blue-600 text-white text-xs font-medium rounded-lg hover:bg-blue-700 transition-colors disabled:opacity-50">
              <Loader2 v-if="installing.has(server.id)" :size="12" class="animate-spin" />
              <Download v-else :size="12" />
              {{ installing.has(server.id) ? '安装中' : '安装' }}
            </button>
          </div>
        </div>
      </div>

      <!-- Empty State -->
      <div v-else class="text-center py-24">
        <Server :size="48" class="mx-auto mb-4 text-gray-300" />
        <p class="text-sm text-gray-500">未找到匹配的 MCP Server</p>
      </div>
    </div>
  </div>
</template>
