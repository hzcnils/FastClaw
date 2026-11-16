<template>
  <div class="pull-kanban">
    <!-- 顶部欢迎区域 -->
    <div class="welcome-section">
      <h1 class="welcome-title">{{ $t('kanban.title') }}</h1>
    </div>

    <!-- 统计卡片区域 -->
    <div class="stats-grid">
      <div class="stats-card">
        <div class="card-content-left">
          <div class="card-header">
            <svg class="card-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <rect x="3" y="3" width="18" height="18" rx="2" />
              <line x1="3" y1="9" x2="21" y2="9" />
              <line x1="9" y1="21" x2="9" y2="9" />
            </svg>
            <span class="card-title">{{ $t('kanban.accountData') }}</span>
          </div>
          <div class="stat-items">
            <div class="stat-item">
              <div class="stat-dot blue"></div>
              <div class="stat-info">
                <div class="stat-label">{{ $t('kanban.availableQuota') }}</div>
                <div class="stat-value">￥{{ availableQuota }}</div>
              </div>
            </div>
            <div class="stat-item">
              <div class="stat-dot purple"></div>
              <div class="stat-info">
                <div class="stat-label">{{ $t('kanban.usedQuota') }}</div>
                <div class="stat-value">￥{{ usedQuota }}</div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div class="stats-card">
        <div class="card-content-left">
          <div class="card-header">
            <svg class="card-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="22 12 18 12 15 21 9 3 6 12 2 12" />
            </svg>
            <span class="card-title">{{ $t('kanban.usageStats') }}</span>
          </div>
          <div class="stat-items">
            <div class="stat-item">
              <div class="stat-dot green"></div>
              <div class="stat-info">
                <div class="stat-label">{{ $t('kanban.requestCount') }}</div>
                <div class="stat-value">{{ totalRequestCount }}</div>
              </div>
            </div>
            <div class="stat-item">
              <div class="stat-dot cyan"></div>
              <div class="stat-info">
                <div class="stat-label">{{ $t('kanban.todayRequestCount') }}</div>
                <div class="stat-value">{{ todayRequestCount }}</div>
              </div>
            </div>
          </div>
        </div>
        <div class="card-content-right">
          <svg class="trend-line" viewBox="0 0 100 50" preserveAspectRatio="none">
            <polyline points="0,40 15,32 30,35 45,22 60,26 75,15 90,10 100,5" fill="none" stroke="#22c55e"
              stroke-width="2" />
          </svg>
        </div>
      </div>

      <div class="stats-card">
        <div class="card-content-left">
          <div class="card-header">
            <svg class="card-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M12 2v20M2 12h20" />
            </svg>
            <span class="card-title">{{ $t('kanban.resourceUsage') }}</span>
          </div>
          <div class="stat-items">
            <div class="stat-item">
              <div class="stat-dot yellow"></div>
              <div class="stat-info">
                <div class="stat-label">{{ $t('kanban.totalQuota') }}</div>
                <div class="stat-value">￥{{ totalQuotaCNY.toFixed(2) }}</div>
              </div>
            </div>
            <div class="stat-item">
              <div class="stat-dot red"></div>
              <div class="stat-info">
                <div class="stat-label">{{ $t('kanban.totalTokens') }}</div>
                <div class="stat-value">{{ totalTokensCount.toLocaleString() }}</div>
              </div>
            </div>
          </div>
        </div>
        <div class="card-content-right">
          <svg class="trend-line" viewBox="0 0 100 50" preserveAspectRatio="none">
            <polyline points="0,10 15,18 30,14 45,25 60,22 75,32 90,38 100,42" fill="none" stroke="#ec4899"
              stroke-width="2" />
          </svg>
        </div>
      </div>

      <div class="stats-card">
        <div class="card-content-left">
          <div class="card-header">
            <svg class="card-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <circle cx="12" cy="12" r="10" />
              <polyline points="12 6 12 12 16 14" />
            </svg>
            <span class="card-title">{{ $t('kanban.performanceMetrics') }}</span>
          </div>
          <div class="stat-items">
            <div class="stat-item">
              <div class="stat-dot indigo"></div>
              <div class="stat-info">
                <div class="stat-label">{{ $t('kanban.avgTTFT') }}</div>
                <div class="stat-value">0.004</div>
              </div>
            </div>
            <div class="stat-item">
              <div class="stat-dot orange"></div>
              <div class="stat-info">
                <div class="stat-label">{{ $t('kanban.avgTPS') }}</div>
                <div class="stat-value">4.843</div>
              </div>
            </div>
          </div>
        </div>
        <div class="card-content-right">
          <svg class="trend-line" viewBox="0 0 100 50" preserveAspectRatio="none">
            <polyline points="0,40 15,35 30,30 45,25 60,20 75,15 90,10 100,5" fill="none" stroke="#f59e0b"
              stroke-width="2" />
          </svg>
        </div>
      </div>
    </div>

    <!-- 图表区域：左右结构 -->
    <div class="chart-section">
      <!-- 左侧：{{ $t('kanban.modelDataAnalysis') }} -->
      <div class="chart-left">
        <div class="section-header">
          <div class="section-title-left">
            <svg class="section-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M3 3v18h18" />
              <path d="M18.7 8l-5.1 5.2-2.8-2.7L7 14.3" />
            </svg>
            <span class="section-title">{{ $t('kanban.modelDataAnalysis') }}</span>
          </div>
          <div class="section-tabs">
            <span v-for="tab in tabs" :key="tab" class="tab" :class="{ active: activeTab === tab }"
              @click="switchTab(tab)">{{ $t(tabLabelMap[tab]) }}</span>
          </div>
        </div>
        <div class="chart-card main-chart">
          <div class="chart-header">
            <h3 class="chart-title">{{ $t(chartTitle) }}</h3>
            <span class="chart-subtitle">{{ chartSubtitle }}</span>
          </div>

          <!-- ECharts 图表内容：四个实例通过 v-show 切换 -->
          <div class="chart-content echarts-content">
            <!-- 无数据空状态 -->
            <div v-if="!hasChartData" class="chart-empty-state">
              <svg viewBox="0 0 1417 1024" width="140" height="100" xmlns="http://www.w3.org/2000/svg">
                <path d="M1393.19527 860.372656c-7.383905-51.687338-40.168445-102.193251-89.78829-117.84713 20.379579 52.868763 21.265648 111.349293 2.953562 164.808769-7.088549 20.674935-17.426017 42.826651-9.746755 63.20623 5.316412 12.700317 15.358523 22.742429 27.763484 28.354197 12.404961 5.316412 25.695991 9.156043 39.282377 11.223536l2.658206 2.067493c20.084223-48.143063 34.261321-100.71647 27.172772-152.108451z" fill="#F4F4F4"/>
                <path d="M1305.179117 744.888376a202.141794 202.141794 0 0 1 55.822325 114.893568c1.181425 9.156043 1.181425 18.607442-0.590712 27.763484-2.067494 8.860686-6.497837 17.130661-12.700318 23.628498-6.497837 5.907124-12.109605 12.404961-16.835304 19.788866-4.134987 7.974618-4.430343 17.130661-1.181425 25.400635 4.430343 10.042111 10.92818 18.902798 19.198154 26.286703 9.156043 7.974618 16.539948 18.016729 21.265648 29.535621 0.295356 1.476781 2.658206 0.590712 2.36285-0.886068-6.793193-20.379579-27.468128-31.603115-37.805596-49.915201-5.316412-8.269974-6.202481-18.607442-2.658206-27.468128 4.430343-7.679262 10.042111-14.472455 16.539948-20.379579 6.497837-6.202481 11.223536-14.177098 13.881742-22.742428 2.067494-9.156043 2.658206-18.312085 1.772138-27.468128-2.067494-20.970291-7.679262-41.34987-16.539949-60.252668a210.884338 210.884338 0 0 0-40.463801-59.661956c-1.181425-1.181425-2.658206 0.590712-1.476781 1.772137z" fill="#FFFFFF"/>
                <path d="M1349.777906 898.178252c-9.746755-6.793193-16.835304-18.016729-20.379579-31.012403-0.295356-1.181425-2.067494-0.590712-1.772137 0.590713 3.544275 13.586386 11.223536 25.105278 21.265647 32.193827 0.886069 0.590712 1.772137-1.181425 0.886069-1.772137z" fill="#FFFFFF"/>
                <path d="M1158.977791 848.853764l2.362849 1.476781c10.632824 5.907124 20.674935 12.995673 29.830978 20.379579 0.886069 0.590712 1.476781 1.181425 2.067494 1.772137a223.289299 223.289299 0 0 1 54.050187 64.683012c5.316412 9.746755 9.746755 19.788866 13.586386 30.126334 5.021056 14.177098 9.156043 29.830978 18.902798 41.645226 0.886069 1.181425 2.067494 2.36285 3.248918 3.544275l85.653303-1.476781h3.839631s0-1.181425-0.590713-1.476782v-0.886068-0.590713-0.590712c-0.886069-3.248918-1.772137-6.202481-2.953562-9.451399-7.679262-25.105278-18.607442-49.029132-32.489184-71.180848-6.202481-9.746755-12.995673-18.902798-20.674935-27.468128a165.901586 165.901586 0 0 0-55.526968-41.054514c-30.126334-14.767811-65.56908-18.312085-98.648976-9.746755l-2.953563 0.590712z" fill="#F4F4F4"/>
                <path d="M1160.454572 846.195558c42.531295 7.383905 81.518316 26.877416 111.349293 56.413037 6.497837 6.202481 12.109605 13.29103 16.244592 21.265648 3.839631 7.679262 5.316412 16.244592 4.430343 24.809922-1.181425 7.974618-1.476781 15.949236-0.886069 23.923853 1.476781 8.269974 6.793193 15.358523 14.472455 19.788867 9.451399 5.021056 19.788866 8.269974 30.42169 9.451399 11.814249 0.886069 23.628497 4.725699 33.965965 10.632824 1.181425 0.886069 2.36285-0.886069 1.181425-1.772138-17.130661-11.814249-39.873089-8.860686-59.071243-17.13066-9.156043-3.248918-15.949236-10.632824-18.607442-19.198154-1.476781-7.974618-1.476781-16.244592 0-24.514566 0.886069-8.269974 0-16.835304-3.544274-24.514566-3.839631-7.974618-9.156043-15.358523-15.358524-21.561004-14.177098-14.767811-30.717046-27.172772-48.733775-36.624171a223.880012 223.880012 0 0 0-66.750505-23.037784c-1.476781 0-1.772137 1.772137 0 2.067493h0.590712z" fill="#FFFFFF"/>
                <path d="M1252.901067 888.431497c10.337468-9.746755 9.746755-23.923853-1.476781-33.079896-1.181425-0.886069-2.953562 0.295356-1.772137 1.181425 10.632824 8.56533 11.223536 21.561004 1.181425 30.717046-0.886069 0.886069 1.181425 2.067494 2.067493 1.181425z" fill="#FFFFFF"/>
                <path d="M215.610037 0h300.081916c16.244592 0 24.21921 7.974618 24.219209 24.21921v97.762907c0 16.244592-7.974618 24.21921-24.219209 24.21921H215.610037c-16.244592 0-24.21921-7.974618-24.219209-24.21921V24.21921c0-16.244592 7.974618-24.21921 24.219209-24.21921z" fill="#F4F4F4"/>
                <path d="M247.508509 73.248342a28.05884 28.05884 0 1 0 56.117681 0 28.05884 28.05884 0 1 0-56.117681 0zM486.156331 67.341217H333.752524c-4.134987-0.590712-7.383905-5.316412-7.383906-11.223536s3.248918-10.632824 7.383906-11.223536h152.403807c3.248918-0.295356 6.202481 1.772137 7.679262 5.316412 1.772137 3.544275 1.772137 8.269974 0 11.814248s-4.725699 5.611768-7.679262 5.316412z m-44.008076 45.189501h-107.805019c-4.430343 0-7.974618-5.021056-7.974618-11.223536s3.544275-11.223536 7.974618-11.223536h107.805019c4.430343 0 7.974618 5.021056 7.974618 11.223536s-3.544275 11.223536-7.974618 11.223536z" fill="#FFFFFF"/>
                <path d="M24.21921 607.547736h221.517162c16.244592 0 24.21921 7.974618 24.219209 24.219209v108.986444c0 16.244592-7.974618 24.21921-24.219209 24.21921H24.21921c-16.244592 0-24.21921-7.974618-24.21921-24.21921v-108.986444c0-16.244592 7.974618-24.21921 24.21921-24.219209z" fill="#E0EAF7"/>
                <path d="M33.670609 680.796077a28.05884 28.05884 0 1 0 56.117681 0 28.05884 28.05884 0 1 0-56.117681 0zM169.534468 675.184309H132.614941c-5.021056-0.590712-8.860686-5.316412-8.860687-11.223536s3.839631-10.632824 8.860687-11.223536h36.919527c3.544275-0.295356 7.088549 1.772137 9.156043 5.316412 1.772137 3.544275 1.772137 8.269974 0 11.814248s-5.316412 5.611768-9.156043 5.316412z m36.328814 45.189501H131.728872c-4.430343-0.590712-7.974618-5.316412-7.974618-11.223536s3.544275-10.632824 7.974618-11.223536h74.13441c4.430343 0.590712 7.974618 5.316412 7.974618 11.223536s-3.544275 10.632824-7.974618 11.223536z" fill="#FFFFFF"/>
                <path d="M191.390828 1012.776464H1417.709836v11.223536H191.390828v-11.223536z" fill="#E0EAF7"/>
                <path d="M1035.223536 157.424863a33.670609 33.670609 0 1 0 67.341217 0 33.670609 33.670609 0 1 0-67.341217 0z" fill="#F4F4F4"/>
                <path d="M1292.774156 922.988174c-1.772137 4.725699-5.316412 7.974618-9.451399 8.56533-1.181425 0-0.886069 2.36285 0 2.067494 4.430343-0.886069 8.269974-4.430343 10.337468-9.451399v-1.181425c0-0.295356-0.295356-0.590712-0.590712-0.590712 0 0-0.590712 0-0.590713 0.590712h0.295356z" fill="#FFFFFF"/>
                <path d="M1271.508509 309.533314a39.282377 39.282377 0 1 0 78.535218 0.029536 39.282377 39.282377 0 0 0-78.535218-0.029536z" fill="#FBD8C0"/>
                <path d="M1106.995097 697.631382h-219.154312c-17.721373 0-31.898471 14.472455-31.898472 31.898471v13.586386c0 12.700317-10.042111 22.742429-22.742428 22.742428h-191.981541c-12.404961 0-22.742429-10.337468-22.742428-22.742428v-13.881742c0-17.721373-14.177098-31.898471-31.898472-32.193828h-219.449668c-10.042111 0-18.312085 8.269974-18.312085 18.607442v289.153735c0 10.042111 8.269974 18.607442 18.312085 18.607442h739.571964c10.042111 0 18.312085-8.269974 18.312086-18.607442v-289.153735c0-10.042111-8.269974-18.312085-18.312086-18.607442z m8.860686 307.761176c0 5.021056-3.839631 8.860686-8.860686 9.156043H367.423132c-5.021056 0-8.860686-4.134987-8.860686-9.156043v-289.153735c0-2.36285 0.886069-4.725699 2.658206-6.497837s3.839631-2.658206 6.497837-2.658205h219.154312c12.404961 0 22.742429 10.337468 22.742428 22.742428v13.586386c0 17.721373 14.472455 31.898471 32.193828 31.898471h191.686184c17.721373 0 31.898471-14.472455 32.193827-31.898471v-13.586386c0-12.700317 10.042111-22.742429 22.742429-22.742428h219.154312c5.021056 0 8.860686 4.134987 8.860687 9.156042v289.153735z" fill="#D3D3D3"/>
                <path d="M502.105567 191.390828h492.654168c16.835304 1.181425 29.535622 15.949236 29.240265 33.079896V703.538506h-7.679262V225.652149c0.590712-12.995673-8.56533-24.514566-20.379578-25.991347h-493.244881c-12.404961 0.886069-22.151716 11.814249-22.151716 25.105278v478.772426h-7.679262V225.947505c-0.886069-17.426017 11.223536-32.78454 28.058841-34.556677h1.476781z" fill="#CACACC"/>
                <path d="M860.963369 337.592155h-135.273147c-9.451399-0.590712-16.835304-7.974618-16.835304-16.835305s7.383905-16.244592 16.835304-16.835304h135.273147c9.451399 0.590712 16.835304 7.974618 16.835304 16.835304s-7.383905 16.244592-16.835304 16.835305z m66.455148 0h-20.674935c-10.042111-0.590712-17.721373-7.974618-17.721373-16.835305s7.679262-16.244592 17.721373-16.835304h20.674935c10.042111 0.590712 17.721373 7.974618 17.721373 16.835304s-7.679262 16.244592-17.721373 16.835305z" fill="#F4F4F4"/>
                <path d="M651.555812 308.056533H614.340929V270.546294c-0.295356-6.497837-5.907124-11.814249-12.404961-11.814249-6.497837 0-12.109605 5.021056-12.404961 11.814249v37.510239H552.316123c-6.793193 0-12.404961 5.611768-12.404961 12.404961s5.611768 12.404961 12.404961 12.404961h37.214884v37.805596c0.295356 6.497837 5.907124 11.518892 12.404961 11.814248 6.497837 0 12.109605-5.021056 12.404961-11.814248v-37.51024h37.214883c6.793193 0 12.404961-5.611768 12.404961-12.404961s-5.611768-12.404961-12.404961-12.404961z" fill="#326AB3"/>
                <path d="M564.425728 450.122873h367.718489c16.244592 0 24.21921 7.974618 24.21921 24.219209v30.126334c0 16.244592-7.974618 24.21921-24.21921 24.21921H564.425728c-16.244592 0-24.21921-7.974618-24.219209-24.21921v-30.126334c0-16.244592 7.974618-24.21921 24.219209-24.219209zM564.425728 540.206519h142.657053c16.244592 0 24.21921 7.974618 24.219209 24.219209v30.126334c0 16.244592-7.974618 24.21921-24.219209 24.21921h-142.657053c-16.244592 0-24.21921-7.974618-24.219209-24.21921v-30.126334c0-16.244592 7.974618-24.21921 24.219209-24.219209zM146.201327 404.933372a22.447072 22.447072 0 1 0 44.894145 0 22.447072 22.447072 0 0 0-44.894145 0zM1158.977791 528.687626a11.223536 11.223536 0 1 0 22.447072 0 11.223536 11.223536 0 1 0-22.447072 0z" fill="#E0EAF7"/>
                <path d="M1025.181425 532.822613l-0.886069 2.36285-5.611768 5.021056 93.627921 148.859532c2.36285 3.839631 3.544275 8.269974 3.544274 12.700318v303.626189c0 2.36285-0.886069 4.430343-2.36285 6.202481s-3.839631 2.658206-5.907124 2.658206H367.127776c-5.021056 0-8.860686-4.134987-8.860686-9.156043V701.766369c0-4.430343 1.181425-8.860686 3.544274-12.700318l116.370349-165.990193-2.953562-12.995673-121.096048 173.669454c-3.248918 5.316412-5.316412 11.518892-5.021056 18.01673v289.153735c0 18.016729 14.177098 32.78454 31.898471 32.78454h712.399193c17.721373 0 31.898471-14.767811 32.193827-32.78454v-289.153735c0-6.202481-1.772137-12.404961-5.021055-17.721373l-95.104702-151.222383z" fill="#D3D3D3"/>
              </svg>
              <p style="color: var(--text-secondary); margin-top: 12px; font-size: var(--font-size-base);">{{ $t('common.noData') }}</p>
            </div>
            <template v-else>
            <div v-show="activeTab === 'consumptionDist'" class="echart-wrapper">
              <VChart ref="chartRefs.bar" class="echart" :option="barChartOption" :theme="chartTheme" autoresize />
            </div>
            <div v-show="activeTab === 'callCountDist'" class="echart-wrapper pie-layout">
              <div class="pie-legend-panel">
                <div v-for="d in pieData" :key="d.label" class="pie-legend-item">
                  <span class="pie-legend-dot" :style="{ background: d.color }"></span>
                  <span class="pie-legend-label">{{ d.label }}</span>
                </div>
              </div>
              <VChart ref="chartRefs.pie" class="echart pie-chart-area" :option="pieChartOption" :theme="chartTheme" autoresize />
            </div>
            <div v-show="activeTab === 'consumptionTrend'" class="echart-wrapper">
              <VChart
                :key="'line-' + themeKey"
                ref="chartRefs.line"
                class="echart"
                :option="lineChartOption"
                :theme="chartTheme"
                :update-options="{ notMerge: true }"
                autoresize
              />
            </div>
            <div v-show="activeTab === 'callCountRank'" class="echart-wrapper rank-scroll-wrapper">
              <VChart ref="chartRefs.rank" class="echart rank-chart" :style="{ height: rankChartHeight + 'px' }" :option="rankChartOption" :theme="chartTheme" autoresize />
            </div>
            </template>
          </div>
        </div>
      </div>

      <!-- 右侧：{{ $t('kanban.apiInfo') }} -->
      <!-- <div class="chart-right">
        <div class="side-chart-header">
          <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="12" cy="12" r="10" />
            <line x1="12" y1="16" x2="12" y2="12" />
            <line x1="12" y1="8" x2="12.01" y2="8" />
          </svg>
          <span class="side-chart-title">{{ $t('kanban.apiInfo') }}</span>
        </div>
        <div class="chart-card side-chart">
          <div class="empty-state">
            <div class="empty-icon">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                <rect x="3" y="3" width="18" height="18" rx="2" ry="2" />
                <line x1="9" y1="9" x2="15" y2="15" />
                <line x1="15" y1="9" x2="9" y2="15" />
              </svg>
            </div>
            <h4 class="empty-title">{{ $t('kanban.noApiInfo') }}</h4>
            <p class="empty-desc">{{ $t('kanban.contactAdminApi') }}</p>
          </div>
        </div>
      </div> -->
    </div>

    <!-- 底部信息区域 -->
    <!-- <div class="info-grid">
      <div class="info-card">
        <div class="info-header">
          <div class="info-title-wrapper">
            <svg class="info-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z" />
              <polyline points="22,6 12,13 2,6" />
            </svg>
            <span class="info-title">{{ $t('kanban.systemNotice') }}</span>
          </div>
          <span class="info-badge">{{ $t('kanban.recentThreeMonths') }}</span>
        </div>
        <div class="empty-state small">
          <div class="empty-icon small">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
              <rect x="3" y="3" width="18" height="18" rx="2" ry="2" />
              <line x1="9" y1="9" x2="15" y2="15" />
              <line x1="15" y1="9" x2="9" y2="15" />
            </svg>
          </div>
          <h4 class="empty-title">{{ $t('kanban.noSystemNotice') }}</h4>
          <p class="empty-desc">{{ $t('kanban.contactAdminNotice') }}</p>
        </div>
      </div>

      <div class="info-card">
        <div class="info-header">
          <div class="info-title-wrapper">
            <svg class="info-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <circle cx="12" cy="12" r="10" />
              <path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3" />
              <line x1="12" y1="17" x2="12.01" y2="17" />
            </svg>
            <span class="info-title">{{ $t('kanban.faq') }}</span>
          </div>
          <div class="status-dots">
            <span class="dot default"></span>
            <span class="dot processing"></span>
            <span class="dot success"></span>
            <span class="dot warning"></span>
            <span class="dot error"></span>
          </div>
        </div>
        <div class="empty-state small">
          <div class="empty-icon small">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
              <rect x="3" y="3" width="18" height="18" rx="2" ry="2" />
              <line x1="9" y1="9" x2="15" y2="15" />
              <line x1="15" y1="9" x2="9" y2="15" />
            </svg>
          </div>
          <h4 class="empty-title">{{ $t('kanban.noFaq') }}</h4>
          <p class="empty-desc">{{ $t('kanban.contactAdminFaq') }}</p>
        </div>
      </div>

      <div class="info-card">
        <div class="info-header">
          <div class="info-title-wrapper">
            <svg class="info-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14" />
              <polyline points="22 4 12 14.01 9 11.01" />
            </svg>
            <span class="info-title">{{ $t('kanban.serviceAvailability') }}</span>
          </div>
          <svg class="refresh-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polyline points="23 4 23 10 17 10" />
            <path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10" />
          </svg>
        </div>
        <div class="empty-state small">
          <div class="empty-icon small">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
              <rect x="3" y="3" width="18" height="18" rx="2" ry="2" />
              <line x1="9" y1="9" x2="15" y2="15" />
              <line x1="15" y1="9" x2="9" y2="15" />
            </svg>
          </div>
          <h4 class="empty-title">{{ $t('kanban.noMonitoringData') }}</h4>
          <p class="empty-desc">{{ $t('kanban.contactAdminUptime') }}</p>
        </div>
      </div>
    </div> -->
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onActivated, watch, nextTick } from 'vue'
import { useI18n } from 'vue-i18n'
import { useThemeStore } from '@/stores/theme'
import newApi from '@/api/newApi'
import VChart from 'vue-echarts'
import { use } from 'echarts/core'
import { BarChart, LineChart, PieChart } from 'echarts/charts'
import { GridComponent, TooltipComponent, LegendComponent } from 'echarts/components'
import { CanvasRenderer } from 'echarts/renderers'

use([BarChart, LineChart, PieChart, GridComponent, TooltipComponent, LegendComponent, CanvasRenderer])

const { t } = useI18n()
const themeStore = useThemeStore()

// 主题变化计数器：确保图表在 draft 预览或 confirm 后都能重新计算颜色
const themeKey = ref(0)
watch(() => themeStore.draft, () => { themeKey.value++ }, { deep: true })
watch(() => themeStore.current, () => { themeKey.value++ }, { deep: true })

// 管理员额度：可用额度 balance、已用额度 totalConsume
const availableQuota = ref('0.00')
const usedQuota = ref('0.00')

// 请求次数
const totalRequestCount = ref(0)
const todayRequestCount = ref(0)

// 总额度
const totalQuota = ref(0)

// 总 Token 量（从图表数据汇总）
const totalTokensCount = ref(0)

// 总记额度 CNY = totalQuota × price / quota_per_unit
const totalQuotaCNY = computed(() => {
  return (totalQuota.value * price.value) / quotaPerUnit.value
})

// 系统配置：价格换算
const price = ref(7.3)
const quotaPerUnit = ref(500000)

// ========== 系统配置获取 ==========
const fetchSystemConfig = async () => {
  try {
    const res = await newApi.sysApi.status()
    if (res && res.data) {
      price.value = res.data.price || 7.3
      quotaPerUnit.value = res.data.quota_per_unit || 500000
    }
  } catch (err) {
    console.error('[PullKanban] 获取系统配置失败:', err)
  }
}

// ========== 用户数据获取 ==========
const fetchUserData = async () => {
  try {
    const res = await newApi.userApi.self()
    if (res && res.data) {
      totalRequestCount.value = res.data.request_count || 0
      totalQuota.value = res.data.quota || 0
    }
  } catch (err) {
    console.error('[PullKanban] 获取用户数据失败:', err)
  }
}

// 计算今日时间戳
const getTodayTimestamps = () => {
  const now = new Date()
  const endTimestamp = Math.floor(now.getTime() / 1000)
  const startOfDay = new Date(now.getFullYear(), now.getMonth(), now.getDate())
  const startTimestamp = Math.floor(startOfDay.getTime() / 1000)
  return { startTimestamp, endTimestamp }
}

// 计算近N天时间戳
const getRangeTimestamps = (days) => {
  const now = new Date()
  const endTimestamp = Math.floor(now.getTime() / 1000)
  const startDate = new Date(now)
  startDate.setDate(startDate.getDate() - (days - 1))
  startDate.setHours(0, 0, 0, 0)
  const startTimestamp = Math.floor(startDate.getTime() / 1000)
  return { startTimestamp, endTimestamp }
}

// ========== 今日请求次数获取 ==========
const fetchTodayRequestCount = async () => {
  try {
    const { startTimestamp, endTimestamp } = getTodayTimestamps()
    const res = await newApi.dataApi.self({
      start_timestamp: startTimestamp,
      end_timestamp: endTimestamp,
      default_time: 'day'
    })
    if (res && res.data && Array.isArray(res.data)) {
      todayRequestCount.value = res.data.reduce((sum, item) => sum + (item.count || 0), 0)
    }
  } catch (err) {
    console.error('[PullKanban] 获取今日请求次数失败:', err)
  }
}

function fetchAllData() {
  fetchUserData()
  fetchTodayRequestCount()
  fetchSystemConfig()
  fetchChartData()
}

onMounted(fetchAllData)

// keep-alive 激活时刷新数据
onActivated(fetchAllData)

// 当前选中的选项卡
const activeTab = ref('consumptionDist')

// 选项卡列表
const tabs = ['consumptionDist', 'consumptionTrend', 'callCountDist', 'callCountRank']
const tabLabelMap = {
  'consumptionDist': 'kanban.consumptionDist',
  'consumptionTrend': 'kanban.consumptionTrend',
  'callCountDist': 'kanban.callCountDist',
  'callCountRank': 'kanban.callCountRank'
}
const tabChartTitleMap = {
  'consumptionDist': 'kanban.modelConsumptionDist',
  'consumptionTrend': 'kanban.modelConsumptionTrend',
  'callCountDist': 'kanban.modelCallCountRatio',
  'callCountRank': 'kanban.modelCallCountRank'
}

// 切换选项卡
const switchTab = (tab) => {
  activeTab.value = tab
}

// 颜色池
const chartColors = ['#22c55e', '#9ca3af', '#f59e0b', '#fbbf24', '#3b82f6', '#ef4444', '#8b5cf6', '#06b6d4']

// 消耗数据（消耗分布和消耗趋势共用）
const consumptionData = ref([])

// 饼图数据
const pieData = ref([])

// 折线图模型列表
const lineModels = ref([])

// 获取图表数据（近7天）
const fetchChartData = async () => {
  try {
    const { startTimestamp, endTimestamp } = getRangeTimestamps(7)
    const res = await newApi.dataApi.self({
      start_timestamp: startTimestamp,
      end_timestamp: endTimestamp,
      default_time: 'day'
    })

    if (res && res.data && Array.isArray(res.data)) {
      const data = res.data

      // 无原始数据：清空图表，走缺页空状态
      if (data.length === 0) {
        consumptionData.value = []
        pieData.value = []
        lineModels.value = []
        totalTokensCount.value = 0
        return
      }

      // 汇总总 Token 量
      totalTokensCount.value = data.reduce((sum, item) => sum + (item.token_used || 0), 0)

      // 提取唯一模型名称并汇总
      const modelAgg = {}
      data.forEach(d => {
        if (!modelAgg[d.model_name]) {
          modelAgg[d.model_name] = { count: 0, quota: 0, tokenUsed: 0 }
        }
        modelAgg[d.model_name].count += d.count || 0
        modelAgg[d.model_name].quota += d.quota || 0
        modelAgg[d.model_name].tokenUsed += d.token_used || 0
      })

      const modelNames = Object.keys(modelAgg)

      // 动态构建 lineModels
      lineModels.value = modelNames.map((name, i) => ({
        key: name,
        color: chartColors[i % chartColors.length],
        label: name
      }))

      // 生成7天标签
      const now = new Date()
      const dayLabels = []
      for (let i = 6; i >= 0; i--) {
        const d = new Date(now)
        d.setDate(d.getDate() - i)
        dayLabels.push({
          label: `${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`,
          timestamp: Math.floor(new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime() / 1000)
        })
      }

      // 构建 consumptionData（models=token_used 显示用, quotaModels=quota 计算CNY用）
      consumptionData.value = dayLabels.map(day => {
        const models = {}
        const quotaModels = {}
        modelNames.forEach(name => {
          models[name] = 0
          quotaModels[name] = 0
        })

        data.forEach(item => {
          const itemDate = new Date(item.created_at * 1000)
          const itemDayTs = Math.floor(new Date(itemDate.getFullYear(), itemDate.getMonth(), itemDate.getDate()).getTime() / 1000)

          if (itemDayTs === day.timestamp) {
            models[item.model_name] = (models[item.model_name] || 0) + (item.token_used || 0)
            quotaModels[item.model_name] = (quotaModels[item.model_name] || 0) + (item.quota || 0)
          }
        })

        return { time: day.label, models, quotaModels }
      })

      // 构建 pieData（value=count 调用次数作Y轴, tokenUsed=Token展示, quota=CNY计算）
      pieData.value = modelNames
        .map((name, i) => ({
          label: name,
          value: modelAgg[name].count,
          tokenUsed: modelAgg[name].tokenUsed,
          quota: modelAgg[name].quota,
          count: modelAgg[name].count,
          color: chartColors[i % chartColors.length]
        }))
        .filter(item => item.value > 0)
        .sort((a, b) => b.value - a.value)
    }
  } catch (err) {
    console.error('[PullKanban] 获取图表数据失败:', err)
  }
}

// 图表标题
const chartTitle = computed(() => {
  return tabChartTitleMap[activeTab.value] || 'kanban.modelConsumptionDist'
})

// 图表副标题
const chartSubtitle = computed(() => {
  if (activeTab.value === 'consumptionDist' || activeTab.value === 'consumptionTrend') {
    const totalTokens = consumptionData.value.reduce((sum, d) => sum + Object.values(d.models).reduce((s, v) => s + v, 0), 0)
    const totalQuota = consumptionData.value.reduce((sum, d) => sum + Object.values(d.quotaModels).reduce((s, v) => s + v, 0), 0)
    return `${t('kanban.total')}: ${totalTokens.toLocaleString()} Token / ¥${Number(formatQuotaCNY(totalQuota)).toFixed(2)}`
  } else {
    const totalCount = pieData.value.reduce((sum, d) => sum + d.count, 0)
    const totalQuota = pieData.value.reduce((sum, d) => sum + d.quota, 0)
    return `${t('kanban.total')}: ${totalCount.toLocaleString()} 次 / ¥${Number(formatQuotaCNY(totalQuota)).toFixed(2)}`
  }
})

// ========== ECharts 主题适配 ==========
const chartTheme = computed(() => {
  const isDark = typeof document !== 'undefined' && document.documentElement.dataset.theme === 'dark'
  return isDark ? 'dark' : undefined
})

const resolveCssVar = (varName) => {
  if (typeof document === 'undefined') return ''
  return getComputedStyle(document.documentElement).getPropertyValue(varName).trim()
}

// 默认颜色池
const rankColors = ['#22c55e', '#9ca3af', '#f59e0b', '#fbbf24', '#3b82f6', '#ef4444', '#8b5cf6', '#06b6d4']

// 调用次数排行数据（从 pieData 计算得出）
const rankData = computed(() => {
  const data = pieData.value
  if (!data.length) return []
  const maxValue = Math.max(...data.map(d => d.value), 1)
  return [...data]
    .sort((a, b) => b.value - a.value)
    .map((item, index) => ({
      label: item.label,
      value: item.value,
      tokenUsed: item.tokenUsed || 0,
      quota: item.quota || item.value,
      count: item.count || 0,
      color: item.color || rankColors[index % rankColors.length],
      percent: Math.round((item.value / maxValue) * 100)
    }))
})

// ========== ECharts 图表选项 ==========

// ========== CNY 换算工具函数 ==========
// 公式: 额度(元) = quota × price / quota_per_unit
const calcQuotaToCNY = (quota) => {
  return (quota * price.value) / quotaPerUnit.value
}

const formatQuotaCNY = (quota) => {
  return calcQuotaToCNY(quota).toFixed(4)
}

// 消耗分布 - 分组柱状图
// 图例每行约 24px（11px 字体 + itemGap），与图间距 10px，x轴区域约 20px，超出最大行数才滚动
const LEGEND_ROW_HEIGHT = 24
const LEGEND_MAX_ROWS = 3
const AXIS_AREA = 20
const LEGEND_GAP = 10

function calcLegendType(modelCount) {
  const rows = Math.ceil(modelCount / 7)
  return rows > LEGEND_MAX_ROWS ? 'scroll' : 'plain'
}

function calcGridBottom(modelCount) {
  const rows = Math.min(LEGEND_MAX_ROWS, Math.max(1, Math.ceil(modelCount / 7)))
  return AXIS_AREA + LEGEND_GAP + rows * LEGEND_ROW_HEIGHT
}
const barChartOption = computed(() => {
  void themeKey.value
  const data = consumptionData.value
  const models = lineModels.value
  if (!data.length || !models.length) return {}
  const legendBottom = calcGridBottom(models.length)
  const legendType = calcLegendType(models.length)
  return {
    tooltip: {
      trigger: 'axis',
      axisPointer: { type: 'shadow' },
      appendToBody: true,
      confine: true,
      backgroundColor: resolveCssVar('--theme-dailog-color') || '#fff',
      borderColor: resolveCssVar('--border-color') || '#e5e7eb',
      textStyle: { color: resolveCssVar('--text-primary') || '#333', fontSize: 12 },
  enterable: true, // ✅ 关键！开启鼠标可移入tooltip，滚轮才能生效
 extraCssText: `
    max-height: 400px;
    overflow-y: auto;
    overflow-x: hidden; /* 禁止水平滚动条 */
    max-width: 480px;   /* 调宽提示框，按需修改 */
    scrollbar-width: thin;
      scrollbar-color: #c0c4cc transparent;
  `,
  position: function (point) {
    const tipW = 480 // 和上面max-width保持一致！
    const tipH = 400
    const gap = 16
    let x = point[0] + gap
    let y = point[1] + gap
    // 右侧空间不足，tooltip放到鼠标左侧
    if (x + tipW > document.documentElement.clientWidth) {
      x = point[0] - tipW - gap
    }
    // 下方空间不足，tooltip放到鼠标上方
    if (y + tipH > document.documentElement.clientHeight) {
      y = point[1] - tipH - gap
    }
    return [x, y]
  },
  // 鼠标跟随模式，防止tooltip挡住鼠标
  triggerOn: 'mousemove',
  // ===========================

      formatter: (params) => {
        let html = `<div style="font-weight:600;margin-bottom:6px;padding-bottom:6px;border-bottom:1px solid ${resolveCssVar('--border-color') || '#e5e7eb'}">${params[0].axisValue}</div>`
        params.forEach(p => {
          if (p.value > 0) {
            const quota = p.data?.quota ?? p.value
            const cny = formatQuotaCNY(quota)
            html += `<div style="display:flex;align-items:center;gap:8px;margin:3px 0">
              <span style="display:inline-block;width:8px;height:8px;border-radius:50%;background:${p.color};flex-shrink:0"></span>
              <span style="flex:1">${p.seriesName}</span>
              <span style="font-weight: 600">${p.value.toLocaleString()} Token / ¥${cny}</span>
            </div>`
          }
        })
        return html
      }
    },
    legend: {
      data: models.map(m => m.label),
      type: legendType,
      bottom: 0,
      itemGap: 10,
      textStyle: { color: resolveCssVar('--text-secondary') || '#64748b', fontSize: 11 },
      pageTextStyle: { color: resolveCssVar('--text-secondary') || '#64748b' },
      pageIconColor: resolveCssVar('--color-primary') || '#3b82f6',
      pageIconInactiveColor: resolveCssVar('--border-color') || '#d1d5db',
    },
    grid: { top: 10, right: 10, bottom: legendBottom, left: 50 },
    xAxis: {
      type: 'category',
      data: data.map(d => d.time),
      axisLabel: { color: resolveCssVar('--text-secondary') || '#64748b', fontSize: 10 },
      axisLine: { lineStyle: { color: resolveCssVar('--text-secondary') || '#e5e7eb' } },
      axisTick: { lineStyle: { color: resolveCssVar('--text-secondary') || '#e5e7eb' } },
    },
    yAxis: {
      type: 'value',
      axisLabel: { color: resolveCssVar('--text-secondary') || '#64748b', fontSize: 10 }
    },
    series: models.map(m => ({
      name: m.label,
      type: 'bar',
      data: data.map(d => ({ value: d.models[m.key] || 0, quota: d.quotaModels[m.key] || 0 })),
      itemStyle: { color: m.color, borderRadius: [2, 2, 0, 0] },
      barMaxWidth: 20,
      barGap: '20%'
    }))
  }
})

// 消耗趋势 - 折线图
const lineChartOption = computed(() => {
  void themeKey.value
  const data = consumptionData.value
  const models = lineModels.value
  if (!data.length || !models.length) return {}
  const legendBottom = calcGridBottom(models.length)
  const legendType = calcLegendType(models.length)
  return {
    tooltip: {
      trigger: 'axis',
      appendToBody: true,
      confine: true,
      backgroundColor: resolveCssVar('--theme-dailog-color') || '#fff',
      borderColor: resolveCssVar('--border-color') || '#e5e7eb',
      textStyle: { color: resolveCssVar('--text-primary') || '#333', fontSize: 12 },
      formatter: (params) => {
        let html = `<div style="font-weight:600;margin-bottom:6px;padding-bottom:6px;border-bottom:1px solid ${resolveCssVar('--border-color') || '#e5e7eb'}">${params[0].axisValue}</div>`
        params.forEach(p => {
          if (p.value > 0) {
            const quota = p.data?.quota ?? p.value
            const cny = formatQuotaCNY(quota)
            html += `<div style="display:flex;align-items:center;gap:8px;margin:3px 0">
              <span style="display:inline-block;width:8px;height:8px;border-radius:50%;background:${p.color};flex-shrink:0"></span>
              <span style="flex:1">${p.seriesName}</span>
              <span style="font-weight:600">${p.value.toLocaleString()} Token / ¥${cny}</span>
            </div>`
          }
        })
        return html
      }
    },
    legend: {
      data: models.map(m => m.label),
      type: legendType,
      bottom: 0,
      itemGap: 10,
      textStyle: { color: resolveCssVar('--text-secondary') || '#64748b', fontSize: 11 },
      pageTextStyle: { color: resolveCssVar('--text-secondary') || '#64748b' },
      pageIconColor: resolveCssVar('--color-primary') || '#3b82f6',
      pageIconInactiveColor: resolveCssVar('--border-color') || '#d1d5db',
    },
    grid: { top: 10, right: 20, bottom: legendBottom, left: 50 },
    xAxis: {
      type: 'category',
      data: data.map(d => d.time),
      boundaryGap: false,
      // 与页面文字同色（同时设 label/line/tick，避免 ECharts dark 主题覆盖）
      axisLabel: {
        color: resolveCssVar('--text-color-light-black') || resolveCssVar('--text-primary') || '#ffffff',
        fontSize: 10,
      },
      axisLine: {
        show: true,
        lineStyle: {
          color: resolveCssVar('--text-color-light-black') || resolveCssVar('--text-primary') || '#ffffff',
        },
      },
      axisTick: {
        show: true,
        lineStyle: {
          color: resolveCssVar('--text-color-light-black') || resolveCssVar('--text-primary') || '#ffffff',
        },
      },
    },
    yAxis: {
      type: 'value',
      axisLabel: { color: resolveCssVar('--text-secondary') || '#64748b', fontSize: 10 }
    },
    series: models.map(m => ({
      name: m.label,
      type: 'line',
      smooth: true,
      showSymbol: true,
      symbol: 'circle',
      symbolSize: 5,
      data: data.map(d => ({ value: d.models[m.key] || 0, quota: d.quotaModels[m.key] || 0 })),
      lineStyle: { color: m.color, width: 2 },
      itemStyle: { color: m.color }
    }))
  }
})

// 调用次数分布 - 环形图
const pieChartOption = computed(() => {
  void themeKey.value
  const data = pieData.value
  if (!data.length) return {}
  return {
    tooltip: {
      trigger: 'item',
      appendToBody: true,
      confine: true,
      backgroundColor: resolveCssVar('--theme-dailog-color') || '#fff',
      borderColor: resolveCssVar('--border-color') || '#e5e7eb',
      textStyle: { color: resolveCssVar('--text-primary') || '#333', fontSize: 12 },
      formatter: (params) => {
        const count = params.data?.count ?? params.value
        return `${params.name}<br/>调用次数: ${count}`
      }
    },
    legend: { show: false },
    series: [{
      type: 'pie',
      radius: ['42%', '68%'],
      center: ['55%', '50%'],
      avoidLabelOverlap: false,
      itemStyle: { borderRadius: 4, borderColor: 'transparent', borderWidth: 2 },
      label: { show: false },
      emphasis: {
        label: { show: true, fontSize: 14, fontWeight: 'bold' },
        scaleSize: 8
      },
      labelLine: { show: false },
      data: data.map(d => ({ name: d.label, value: d.value, count: d.count, tokenUsed: d.tokenUsed, quota: d.quota, itemStyle: { color: d.color } }))
    }]
  }
})

// 调用次数排行 - 横向柱状图
const rankChartOption = computed(() => {
  void themeKey.value
  const data = rankData.value
  if (!data.length) return {}
  return {
    tooltip: {
      trigger: 'axis',
      axisPointer: { type: 'shadow' },
      appendToBody: true,
      confine: true,
      backgroundColor: resolveCssVar('--theme-dailog-color') || '#fff',
      borderColor: resolveCssVar('--border-color') || '#e5e7eb',
      textStyle: { color: resolveCssVar('--text-primary') || '#333', fontSize: 12 },
      formatter: (params) => {
        const p = params[0]
        const count = p.data?.count ?? p.value
        return `${p.name}<br/>调用次数: ${count}`
      }
    },
    grid: { top: 5, right: 10, bottom: 5, left: 140 },
    xAxis: {
      type: 'value',
      axisLabel: { color: resolveCssVar('--text-secondary') || '#64748b', fontSize: 10 }
    },
    yAxis: {
      // type: 'category',
      // data: data.map((d, i) => `${i + 1}. ${d.label}`),
      // axisLabel: { color: resolveCssVar('--text-primary') || '#1f2937', fontSize: 11 },
      // inverse: true
       type: 'category',
  data: data.map((d, i) => `${i + 1}. ${d.label}`),
  axisLabel: { 
    color: resolveCssVar('--text-secondary') || '#64748b', 
    fontSize: 11 
  },
  inverse: true,
  // Y轴竖线颜色
  axisLine: {
    lineStyle: {
      color: resolveCssVar('--text-secondary') || '#64748b'
    }
  },
  // Y轴刻度小横线
  axisTick: {
    lineStyle: {
      color: resolveCssVar('--text-secondary') || '#64748b'
    }
  }
      
    },
    series: [{
      type: 'bar',
      data: data.map(d => ({
        value: d.value,
        tokenUsed: d.tokenUsed,
        quota: d.quota,
        count: d.count,
        itemStyle: { color: d.color, borderRadius: [0, 4, 4, 0] }
      })),
      barWidth: 16,
    }]
  }
})

const rankChartHeight = computed(() => {
  const n = rankData.value.length || 1
  return Math.max(n * 26 + 10, 200)
})

/** 有真实用量才算有图；仅日期轴占位（全 0）视为空，显示缺页图标 */
const hasChartData = computed(() => {
  if (pieData.value.some(d => Number(d.value || d.count) > 0)) return true
  return consumptionData.value.some(d =>
    Object.values(d.models || {}).some(v => Number(v) > 0),
  )
})

// ========== Tab 切换时 ECharts resize ==========
const chartRefs = {
  bar: ref(null),
  line: ref(null),
  pie: ref(null),
  rank: ref(null)
}

watch(activeTab, () => {
  nextTick(() => {
    const refMap = {
      consumptionDist: chartRefs.bar,
      consumptionTrend: chartRefs.line,
      callCountDist: chartRefs.pie,
      callCountRank: chartRefs.rank
    }
    const chartRef = refMap[activeTab.value]
    if (chartRef?.value?.chart) {
      chartRef.value.chart.resize()
    }
  })
})

// 组件属性
const props = defineProps({
  data: {
    type: Object,
    default: () => ({})
  }
})
</script>

<style scoped>
/* 主容器 */
.pull-kanban {
  display: flex;
  flex-direction: column;
  padding: 16px 20px;
  background: var(--bg-primary);
  height: 100%;
  overflow-x: hidden;
  overflow-y: auto;
  box-sizing: border-box;
}

/* 欢迎区域 */
.welcome-section {
  margin-bottom: 16px;
  height: 28px;
}

.welcome-title {
  font-size: var(--font-size-lg);
  font-weight: 700;
  color: var(--text-primary);
  margin: 0;
  line-height: 28px;
}

/* 统计网格 - 固定4列 */
.stats-grid {
  display: flex;
  gap: 16px;
  margin-bottom: 20px;
  width: 100%;
}

/* 统计卡片 - 固定宽度 */
.stats-card {
  flex: 1 1 0;
  min-width: 0;
  height: 140px;
  background: var(--kanban-card-background);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  padding: 16px;
  box-sizing: border-box;
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: 8px;
}

.stats-card:hover {
  box-shadow: 0 2px 12px rgba(0, 0, 0, 0.06);
}

.card-content-left {
  flex: 1;
  min-width: 0;
}

.card-content-right {
  width: 70px;
  height: 50px;
  flex-shrink: 0;
  margin-top: 24px;
}

.card-header {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 12px;
  height: 20px;
}

.card-icon {
  width: 14px;
  height: 14px;
  color: var(--text-secondary);
  flex-shrink: 0;
}

.card-title {
  font-size: var(--font-size-lm);
  font-weight: 700;
  color: var(--text-secondary);
}

.stat-items {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.stat-item {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 36px;
}

.stat-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.stat-dot.blue {
  background: #3b82f6;
}

.stat-dot.purple {
  background: #8b5cf6;
}

.stat-dot.green {
  background: #22c55e;
}

.stat-dot.cyan {
  background: #06b6d4;
}

.stat-dot.yellow {
  background: #fbbf24;
}

.stat-dot.red {
  background: #ef4444;
}

.stat-dot.indigo {
  background: #6366f1;
}

.stat-dot.orange {
  background: #f97316;
}

.stat-info {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.stat-label {
  font-size: var(--font-size-sm);
  color: var(--text-secondary);
  line-height: 16px;
}

.stat-value {
  font-size: var(--font-size-lg);
  font-weight: 700;
  color: var(--text-primary);
  line-height: 20px;
}

.trend-line {
  width: 100%;
  height: 100%;
}

/* 区域标题 */
.section-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 12px;
}

.section-title-left {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-shrink: 0;
}

.section-tabs {
  display: flex;
  align-items: center;
  gap: 4px;
}

.section-tabs .tab {
  padding: 4px 12px;
  font-size: var(--font-size-lm);
  color: var(--text-secondary);
  border-radius: 4px;
  cursor: pointer;
  white-space: nowrap;
  transition: all 0.2s;
}

.section-tabs .tab:hover {
  color: var(--text-primary);
  background: var(--bg-hover);
}

.section-tabs .tab.active {
  color: var(--color-primary);
  background: var(--bg-active);
  font-weight: 700;
}

.section-icon {
  width: 14px;
  height: 14px;
  color: var(--text-secondary);
}

.section-title {
  font-size: var(--font-size-lm);
  font-weight: 700;
  color: var(--text-primary);
}

/* 图表区域 - 左右结构 */
.chart-section {
  display: flex;
  flex-direction: row;
  align-items: stretch;
  gap: 12px;
  margin-bottom: 12px;
  flex: 1;
  min-height: 0;
}

.chart-left {
  flex: 2;
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.chart-right {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.chart-card {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  background: var(--kanban-card-background);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  padding: 16px;
  box-sizing: border-box;
}

.chart-card:hover {
  box-shadow: 0 2px 12px rgba(0, 0, 0, 0.06);
}

.main-chart {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.side-chart {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.side-chart-header {
  display: flex;
  align-items: center;
  gap: 4px;
  margin-bottom: 16px;
  color: var(--text-secondary);
}

.side-chart-title {
  line-height: 1;
  font-size: var(--font-size-lm);
  font-weight: 700;
  color: var(--text-primary);
}

.chart-header {
  margin-bottom: 12px;
  height: 40px;
}

.chart-title {
  font-size: var(--font-size-base);
  font-weight: 700;
  color: var(--text-primary);
  margin: 0 0 4px 0;
}

.chart-subtitle {
  font-size: var(--font-size-lm);
  color: var(--text-secondary);
}

.chart-content {
  flex: 1;
  min-height: 0;
}

.chart-empty-state {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
}

/* ECharts 容器 */
.echarts-content {
  position: relative;
  display: block;
}


.echart-wrapper {
  position: absolute;
  inset: 0;
}

/* 饼图：自定义图例 + 图表并排布局 */
.echart-wrapper.pie-layout {
  display: flex;
  gap: 0;
}

.pie-legend-panel {
  width: 160px;
  flex-shrink: 0;
  overflow-y: auto;
  padding-right: 4px;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.pie-legend-item {
  display: flex;
  align-items: center;
  gap: 6px;
  cursor: pointer;
  padding: 3px 0;
}

.pie-legend-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.pie-legend-label {
  font-size: 11px;
  color: var(--text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pie-chart-area {
  flex: 1;
  min-width: 0;
}

/* 排行图：数据多时用 CSS 滚动 */
.rank-scroll-wrapper {
  overflow-y: auto;
  overflow-x: hidden;
}

.rank-chart {
  width: 100% !important;
}

.echart {
  width: 100%;
  height: 100%;
}

/* 空状态 */
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 24px 16px;
  flex: 1;
}

.empty-state.small {
  padding: 16px 12px;
}

.empty-icon {
  width: 40px;
  height: 40px;
  color: var(--text-secondary);
  margin-bottom: 10px;
  opacity: 0.5;
}

.empty-icon.small {
  width: 32px;
  height: 32px;
}

.empty-icon svg {
  width: 100%;
  height: 100%;
}

.empty-title {
  font-size: var(--font-size-lm);
  font-weight: 700;
  color: var(--text-primary);
  margin: 0 0 4px 0;
}

.empty-desc {
  font-size: var(--font-size-sm);
  color: var(--text-secondary);
  margin: 0;
  text-align: center;
}

/* 底部信息网格 - 固定3列 */
.info-grid {
  display: flex;
  gap: 16px;
  width: 100%;
}

.info-card {
  flex: 0 0 calc(33.333% - 11px);
  width: calc(33.333% - 11px);
  min-width: 300px;
  height: 200px;
  background: var(--kanban-card-background);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  padding: 12px 16px;
  box-sizing: border-box;
  transition: border-color 0.2s, box-shadow 0.2s;
}

.info-card:hover {
  box-shadow: 0 2px 12px rgba(0, 0, 0, 0.06);
}

.info-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
  height: 24px;
}

.info-title-wrapper {
  display: flex;
  align-items: center;
  gap: 6px;
}

.info-icon {
  width: 14px;
  height: 14px;
  color: var(--text-secondary);
}

.info-title {
  font-size: var(--font-size-lm);
  font-weight: 700;
  color: var(--text-primary);
}

.info-badge {
  font-size: var(--font-size-sm);
  color: var(--color-primary);
  background: var(--bg-hover);
  padding: 2px 8px;
  border-radius: 4px;
}

.status-dots {
  display: flex;
  gap: 4px;
}

.status-dots .dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
}

.status-dots .dot.default {
  background: #9ca3af;
}

.status-dots .dot.processing {
  background: #3b82f6;
}

.status-dots .dot.success {
  background: #22c55e;
}

.status-dots .dot.warning {
  background: #f59e0b;
}

.status-dots .dot.error {
  background: #ef4444;
}

.refresh-icon {
  width: 14px;
  height: 14px;
  color: var(--text-secondary);
  cursor: pointer;
}
</style>

<style>
.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .info-badge {
  color: #fff !important;
}

div[id^="echarts-tooltip-"] {
  z-index: 10000 !important;
}
</style>
