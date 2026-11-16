import { computed, ref } from 'vue'
import { defineStore } from 'pinia'

import BlueBackgroundImage from '../assets/theme/BlueTheme/background.png'
import GreenBackgroundImage from '../assets/theme/GreenTheme/background.png'
import GreyBackgroundImage from '../assets/theme/GreyTheme/background.png'
import OrangeBackgroundImage from '../assets/theme/OrangeTheme/background.png'
import PurpleBackgroundImage from '../assets/theme/PurpleTheme/background.png'
import RedBackgroundImage from '../assets/theme/RedTheme/background.png'
import { p } from 'vue-router/dist/router-CWoNjPRp.mjs'

export type ThemeCategory = 'simple' | 'scene'
export type SimpleThemeId = 'simple-light' | 'simple-dark'
export type SceneThemeId =
  | 'scene-tech-blue'
  | 'scene-forest-green'
  | 'scene-warm-sun'
  | 'scene-aurora-purple'
  | 'scene-rose-dusk'
  | 'scene-graphite-cyan'
export type ThemeId = SimpleThemeId | SceneThemeId | 'custom-scene'
export type OpacityKey = 'mainMenu' | 'mainContent' | 'subMenu' | 'topBar'

export interface ThemeOpacity {
  mainMenu: number
  mainContent: number
  subMenu: number
  topBar: number
}

export interface ThemeButtonVariant {
  bg: string
  text: string
  border: string
  hoverBg: string
  hoverText: string
  hoverBorder: string
  disabledBg: string
  disabledText: string
  disabledBorder: string
}

export interface ThemeButtonPalette {
  primary: ThemeButtonVariant
  secondary: ThemeButtonVariant
  danger: ThemeButtonVariant
  warning: ThemeButtonVariant
  ghost: ThemeButtonVariant
}

export interface ThemePalette {
  mode: 'light' | 'dark'
  bgPrimary: string
  kanbanCardBackground: string//调用看板界面的卡片背景颜色
  inputPrimary: string
  bgSidebar: string
  bgTitlebar: string
  bgTabbar: string
  bgSubmenu: string
  bgHover: string
  modelBgHover: string//模型管理界面左侧供应商列表选中悬浮背景色
  bgActive: string
  textPrimary: string
  textSecondary: string
  textThree: string //智能对话AI回答区域的代码注释文字颜色
  borderColor: string
  colorPrimary: string
  colormyWallet: string//我的钱包界面颜色
  colormyWalletBtnHover: string
  colorbackgroundone: string//卡片背景渐变颜色1
  colorbackgroundtow: string//卡片背景渐变颜色2
  colorbackgroundthree: string//卡片背景渐变颜色3


  BtnBackgroundColor: string//按钮背景颜色
  BtnHoverBackgroundColor: string//鼠标悬浮按钮背景颜色 ，主菜单选中背景颜色
  MeunWenzhiActiveColor: string//菜单选中文字颜色
  colorSuccess: string
  colorWarning: string
  colorDanger: string
  appBackground: string
  appBackgroundImage?: string
  buttons?: ThemeButtonPalette
  modalBg: string
  cardBg: string
  questionBubble: string
  xlBoxText: string
  mainChatBackground: string
  asideChatHistory: string
  tabBarBackground: string
  aiAnswerBackground: string
  chatDividerColor: string   //智能对话 分割线颜色（白色20%透明度）
  adBannerColor: string
  convListHoverColor: string//智能对话 历史会话item鼠标悬浮背景色
  convListHoverActiveColor: string//智能对话 历史会话item选中背景色
  convListHoverActiveBorderColor: string//智能对话 历史会话item鼠标选中边框景色
  chatInputSkillChip: string//skill-chip（写周末计划 ，智能对话消息输入框 背景颜色）
  mainMeunActiveBackground: string //主菜单选中背景色
  mainMeunHoverBackground: string//主菜单鼠标悬浮Item背景色
  BackgroundColor: string//知识库列表界面卡片背景颜色
  TextColorLightBlack: string//文字颜色随系统改变
  TabActiveHover: string //应用配置，API接入渠道， 未选中背景色
  ThemeDialogColor: string//主题 弹窗，下拉框背景色
  DropdownHoverColor: string//下拉框 鼠标悬浮色
  webkitScrollbarTHumb: string//ai回答区域滚动条颜色
  CardBackgroundTmd: string//带10%透明度的背景色
  SpecialTextColor: string//特殊文字颜色
  infoambetextcolor: string
  SpecialTiptextColor: string,//特殊提示文字颜色
  VibeCodeSpecialText: string//情景主题下为白色文字，其余为主题色
  treemenuSearchingColor: string//菜单搜索模式下的背景色
  personalInfocardcolor: string//个人信息界面的个人信息卡片背景色
  personalInfocardcolor2: string//个人信息界面的个人信息卡片背景色 渐变色
  tableheaderbgcolor: string//表头背景色
  textcolordanger: string//智能对话里 AI回答气泡里error-content区域的文字颜色
  cancelbtncolor: string 
  cancelbtnhovercolor: string
}

export interface ThemePreview {
  background: string
  surface: string
  sidebar: string
  accent: string
  text: string
}

export interface ThemePreset {
  id: SimpleThemeId | SceneThemeId
  name: string
  type: string
  description: string
  category: ThemeCategory
  defaultOpacity: ThemeOpacity
  palette: ThemePalette
  preview: ThemePreview
}

export interface ThemeSettings {
  themeId: ThemeId
  customBaseThemeId: SceneThemeId
  opacity: ThemeOpacity
}

export const OPACITY_CONTROLS: Array<{
  key: OpacityKey
  label: string
  description: string
}> = [
    {
      key: 'mainMenu',
      label: '主菜单背景透明度',
      description: '控制左侧主菜单背景的不透明程度',
    },
    {
      key: 'mainContent',
      label: '主体背景颜色透明度',
      description: '控制右侧内容区背景的不透明程度',
    },
    {
      key: 'subMenu',
      label: '子菜单背景透明度',
      description: '控制侧边子菜单和层级菜单背景的不透明程度',
    },
    {
      key: 'topBar',
      label: '顶部背景颜色透明度',
      description: '控制标题栏和标签栏背景的不透明程度',
    },
  ]

const STORAGE_KEY = 'fastclaw-theme-settings'

const DEFAULT_OPACITY: ThemeOpacity = {
  mainMenu: 100,
  mainContent: 100,
  subMenu: 100,
  topBar: 100,
}

const DEFAULT_SETTINGS: ThemeSettings = {
  themeId: 'simple-light',
  customBaseThemeId: 'scene-tech-blue',
  opacity: { ...DEFAULT_OPACITY },
}

function createThemeOpacity(
  mainMenu: number,
  mainContent: number,
  subMenu: number,
  topBar: number
): ThemeOpacity {
  return {
    mainMenu,
    mainContent,
    subMenu,
    topBar,
  }
}

function cloneOpacity(opacity: ThemeOpacity): ThemeOpacity {
  return { ...opacity }
}

function rgbToHex(r: number, g: number, b: number): string {
  const clampChannel = (value: number) => Math.min(255, Math.max(0, Math.round(value)))
  return `#${[r, g, b]
    .map(clampChannel)
    .map((value) => value.toString(16).padStart(2, '0'))
    .join('')}`
}

function mixColors(color1: string, color2: string, weight: number): string {
  const from = hexToRgb(color1)
  const to = hexToRgb(color2)
  if (!from || !to) return color1

  const ratio = Math.min(1, Math.max(0, weight))
  return rgbToHex(
    from.r + (to.r - from.r) * ratio,
    from.g + (to.g - from.g) * ratio,
    from.b + (to.b - from.b) * ratio
  )
}

function getRelativeLuminance(color: string): number {
  const rgb = hexToRgb(color)
  if (!rgb) return 0

  const transform = (value: number) => {
    const channel = value / 255
    return channel <= 0.03928
      ? channel / 12.92
      : ((channel + 0.055) / 1.055) ** 2.4
  }

  return 0.2126 * transform(rgb.r) + 0.7152 * transform(rgb.g) + 0.0722 * transform(rgb.b)
}

function getReadableTextColor(background: string): string {
  return getRelativeLuminance(background) > 0.42 ? '#0f172a' : '#ffffff'
}

function createSolidButtonVariant(
  baseColor: string,
  surfaceColor: string,
  textColor: string,
  mode: 'light' | 'dark'
): ThemeButtonVariant {
  const hoverBg = mixColors(baseColor, '#000000', 0.12)

  return {
    bg: baseColor,
    text: getReadableTextColor(baseColor),
    border: baseColor,
    hoverBg,
    hoverText: getReadableTextColor(hoverBg),
    hoverBorder: hoverBg,
    disabledBg: mixColors(baseColor, surfaceColor, mode === 'dark' ? 0.52 : 0.68),
    disabledText: mixColors(textColor, surfaceColor, mode === 'dark' ? 0.36 : 0.54),
    disabledBorder: mixColors(baseColor, surfaceColor, mode === 'dark' ? 0.42 : 0.6),
  }
}

function createSecondaryButtonVariant(
  surfaceColor: string,
  textColor: string,
  borderColor: string,
  accentColor: string,
  mode: 'light' | 'dark'
): ThemeButtonVariant {
  const hoverBg = mixColors(surfaceColor, accentColor, mode === 'dark' ? 0.28 : 0.14)

  return {
    bg: surfaceColor,
    text: textColor,
    border: borderColor,
    hoverBg,
    hoverText: accentColor,
    hoverBorder: accentColor,
    disabledBg: mixColors(surfaceColor, mode === 'dark' ? '#000000' : '#ffffff', mode === 'dark' ? 0.14 : 0.08),
    disabledText: mixColors(textColor, surfaceColor, mode === 'dark' ? 0.42 : 0.5),
    disabledBorder: mixColors(borderColor, surfaceColor, mode === 'dark' ? 0.3 : 0.42),
  }
}

function createGhostButtonVariant(
  textColor: string,
  accentColor: string,
  surfaceColor: string,
  mode: 'light' | 'dark'
): ThemeButtonVariant {
  return {
    bg: 'transparent',
    text: textColor,
    border: 'transparent',
    hoverBg: mode === 'dark' ? withOpacity('#ffffff', 10) : withOpacity(accentColor, 10),
    hoverText: accentColor,
    hoverBorder: 'transparent',
    disabledBg: 'transparent',
    disabledText: mixColors(textColor, surfaceColor, mode === 'dark' ? 0.44 : 0.58),
    disabledBorder: 'transparent',
  }
}

function createButtonPalette(options: {
  mode: 'light' | 'dark'
  surfaceColor: string
  textColor: string
  borderColor: string
  accentColor: string
  dangerColor: string
  warningColor: string
}): ThemeButtonPalette {
  return {
    primary: createSolidButtonVariant(
      options.accentColor,
      options.surfaceColor,
      options.textColor,
      options.mode
    ),
    secondary: createSecondaryButtonVariant(
      options.surfaceColor,
      options.textColor,
      options.borderColor,
      options.accentColor,
      options.mode
    ),
    danger: createSolidButtonVariant(
      options.dangerColor,
      options.surfaceColor,
      options.textColor,
      options.mode
    ),
    warning: createSolidButtonVariant(
      options.warningColor,
      options.surfaceColor,
      options.textColor,
      options.mode
    ),
    ghost: createGhostButtonVariant(
      options.textColor,
      options.accentColor,
      options.surfaceColor,
      options.mode
    ),
  }
}

export const THEME_PRESETS: ThemePreset[] = [
  {
    id: 'simple-light',
    name: '浅色主题',
    type: 'White',
    description: '默认的清爽浅色界面',
    category: 'simple',
    defaultOpacity: createThemeOpacity(100, 100, 100, 100),
    palette: {
      mode: 'light',
      bgPrimary: '#F8FAFC',  //主界面
      kanbanCardBackground: '#F8FAFC',//调用看板界面的卡片背景颜色
      inputPrimary: '#ffffff',
      bgSidebar: '#F8FAFC',  //主菜单
      bgTitlebar: '#F8FAFC',  //顶部
      bgTabbar: '#f0f3f8',
      bgSubmenu: '#ffffff',
      bgHover: 'rgba(15, 23, 42, 0.06)',
      modelBgHover: '#f4f7fc',//模型管理界面左侧供应商列表选中悬浮背景色
      bgActive: 'rgba(24, 144, 255, 0.12)',
      textPrimary: '#1f2937',
      textSecondary: '#64748b',
      textThree: '#64748b',//智能对话AI回答区域的代码注释文字颜色
      // borderColor: '#e2e8f0',   
      borderColor: '#DCE3EA',  //边框颜色
      colorPrimary: '#1677FF',
      colormyWallet:'#f59e0b',//我的钱包界面颜色
      colormyWalletBtnHover: '#d97706',
      colorbackgroundone: '#fffbeb',//卡片背景渐变颜色1
      colorbackgroundtow: '#fef3c7',//卡片背景渐变颜色2
      colorbackgroundthree: '#fde68a',//卡片背景渐变颜色3
     

      BtnBackgroundColor: '#1677FF',//按钮背景颜色
      BtnHoverBackgroundColor: '#4096FF',//鼠标悬浮按钮背景颜色 ，主菜单选中背景颜色
      MeunWenzhiActiveColor: ' #ffffff',//菜单选中文字颜色
      colorSuccess: '#52c41a',
      colorWarning: '#faad14',
      colorDanger: '#ff4d4f',
      appBackground: '#ffffff',
      modalBg: '#ffffff',
      cardBg: '#f8fafc',  // ， 主题选择按钮背景颜色
      questionBubble: '#f8fafc',  //问题气泡背景颜色 
      xlBoxText: '#000',//ai回答翻译下拉框文字颜色
      mainChatBackground: '#ffffff', //智能对话 聊天区背景颜色
      asideChatHistory: '#ffffff',//智能对话 新建对话区域背景颜色
      tabBarBackground: '#ffffff', //智能对话 tab区域背景颜色
      aiAnswerBackground: '#ffffff',  //智能对话 回答背景颜色
      chatDividerColor: 'rgba(0, 0, 0,0.04)', //智能对话 分割线颜色（白色20%透明度）
      adBannerColor: '#F5F7FA',//顶部广告背景
      convListHoverColor: 'rgba(234, 243, 255,0.7)',//智能对话 历史会话item鼠标悬浮背景色
      convListHoverActiveColor: '#EAF3FF',//智能对话 历史会话item选中背景色
      convListHoverActiveBorderColor: '#C9E0FF',//智能对话 历史会话item鼠标选中边框景色
      chatInputSkillChip: '#ffffff',//skill-chip（写周末计划 ，智能对话消息输入框 背景颜色）
      mainMeunActiveBackground: '#4096FF', //主菜单选中背景色
      mainMeunHoverBackground: '#CDE7FF',//主菜单鼠标悬浮Item背景
      BackgroundColor: '#F8FAFC',//知识库列表界面卡片背景颜色
      TextColorLightBlack: '#000',//转为文档编辑的ai回答界面 文字颜色随系统改变
      TabActiveHover: 'rgba(0, 0, 0,0.1)',//应用配置，API接入渠道， 未选中背景
      ThemeDialogColor: 'rgb(255,255,255)',//主题 弹窗，下拉框背景色
      cancelbtncolor: '#f8fafc',
      cancelbtnhovercolor: '#d8e8fc',

      DropdownHoverColor: 'rgba(0, 0, 0,0.05)',//下拉框 鼠标悬浮色
      webkitScrollbarTHumb: 'rgba(0, 0, 0, 0.10)',//浅色主题滚动条（更浅）
      CardBackgroundTmd: '#fff',//带10%透明度的背景色
      SpecialTextColor:'#1677FF',//特殊文字颜色
      infoambetextcolor:'#d97706',
      SpecialTiptextColor:'#eca334',//特殊提示文字颜色
      VibeCodeSpecialText:'#1677FF',//情景主题下为白色文字，其余为主题色
      treemenuSearchingColor:  'rgba(255, 193, 7, 0.15)',//菜单搜索模式下的背景色
      personalInfocardcolor:'#d8e8fb',//个人信息界面的个人信息卡片背景色
      personalInfocardcolor2: '#ffffff',//个人信息界面的个人信息卡片背景色
      tableheaderbgcolor: '#ebeaea',//表头背景色
      textcolordanger: '#FF6B5A',//智能对话里 AI回答气泡里error-content区域的文字颜色
    },
    preview: {
      background: 'linear-gradient(135deg, #ffffff 0%, #eaf4ff 100%)',
      surface: '#ffffff',
      sidebar: '#f5f7fb',
      accent: '#1890ff',
      text: '#1f2937',
    },
  },
  {
    id: 'simple-dark',
    name: '黑暗主题',
    type: 'Black',
    description: '低亮度深色界面',
    category: 'simple',
    defaultOpacity: createThemeOpacity(100, 100, 100, 100),
    palette: {
      mode: 'dark',
      bgPrimary: '#171717',
      kanbanCardBackground: '#171717',//调用看板界面的卡片背景颜色
      inputPrimary: '#171717',
      bgSidebar: '#101113',
      bgTitlebar: '#111318',
      bgTabbar: '#1f2024',
      bgSubmenu: '#1c1d22',
      bgHover: 'rgba(255, 255, 255, 0.08)',
      modelBgHover: 'rgba(255, 255, 255, 0.08)',//模型管理界面左侧供应商列表选中悬浮背景色
      bgActive: 'rgba(59, 130, 246, 0.18)',
      textPrimary: '#f3f4f6',
      textSecondary: '#a3aab7',
           textThree: '#92959b',//智能对话AI回答区域的代码注释文字颜色
      // borderColor: '#2f333b',
      borderColor: 'rgba(255, 255, 255, 0.1)',
      colorPrimary: '#1677FF',
      colormyWallet:'#f59e0b',//我的钱包界面颜色
      colormyWalletBtnHover: '#d97706',
      colorbackgroundone: '#fffbeb',//卡片背景渐变颜色1
      colorbackgroundtow: '#fef3c7',//卡片背景渐变颜色2
      colorbackgroundthree: '#fde68a',//卡片背景渐变颜色3

      BtnBackgroundColor: '#1677FF',//按钮背景颜色
      BtnHoverBackgroundColor: '#4096FF',//鼠标悬浮按钮背景颜色 ，主菜单选中背景颜色

      colorSuccess: '#34d399',
      colorWarning: '#fbbf24',
      colorDanger: '#f87171',
      appBackground: '#171717',
      modalBg: '#202126',
      cardBg: '#24262d',
      questionBubble: '#24262d',
      xlBoxText: '#ffffff',
      mainChatBackground: 'rgba(0, 0, 0, 0)', //智能对话 聊天区背景颜色
      asideChatHistory: 'rgba(0, 0, 0, 0)', //智能对话 聊天区背景颜色
      tabBarBackground: 'rgba(0, 0, 0, 0)', //智能对话 会话历史背景颜色
      aiAnswerBackground: 'rgba(255, 255, 255, 0.05)',  //智能对话 回答背景颜色
      chatDividerColor: 'rgba(255, 255, 255, 0.05)', //智能对话 分割线颜色（白色20%透明度）
      adBannerColor: 'rgba(255, 255, 255, 0)',//顶部广告背景
      convListHoverColor: 'rgba(255, 255, 255, 0.1)',//智能对话 历史会话item鼠标悬浮背
      convListHoverActiveColor: 'rgba(255, 255, 255, 0.1)',//智能对话 历史会话item选中背景色
      convListHoverActiveBorderColor: 'rgba(255, 255, 255, 0.2)',//智能对话 历史会话item鼠标选中边框景色
      chatInputSkillChip: 'rgba(255, 255, 255, 0)',//skill-chip（写周末计划 ，智能对话消息输入框 背景颜色）
      mainMeunActiveBackground: '#FFFFFF', //主菜单选中背景色
      mainMeunHoverBackground: '#686868',//主菜单鼠标悬浮Item背景色
      MeunWenzhiActiveColor: ' #000000',//菜单选中文字颜色
      BackgroundColor: 'rgba(255, 255, 255, 0.1)',//知识库列表界面卡片背景颜色
      TextColorLightBlack: '#d4d4d8',//转为文档编辑的ai回答界面 文字颜色随系统改变
      TabActiveHover: 'rgba(255, 255, 255,0.1)',//应用配置，API接入渠道， 未选中背景
      ThemeDialogColor: 'rgb(42, 43, 47)',//主题 弹窗，下拉框背景色
      cancelbtncolor: 'rgb(73, 76, 85)',
      cancelbtnhovercolor: '#334f77',
      DropdownHoverColor: 'rgba(255, 255, 255,0.1)',//下拉框 鼠标悬浮色
      webkitScrollbarTHumb: 'rgba(255, 255, 255,0.4)',//ai回答区域滚动条颜色
      CardBackgroundTmd: 'rgba(255, 255, 255,0.15)',//带10%透明度的背景色
      SpecialTextColor:'#1677FF',//特殊文字颜色
      infoambetextcolor:'#d97706',
      SpecialTiptextColor:'#b88230',//特殊提示文字颜色
      VibeCodeSpecialText:'#1677FF',//情景主题下为白色文字，其余为主题色
      treemenuSearchingColor:  'rgba(250, 212, 97, 0.45)',//菜单搜索模式下的背景色
   //   personalInfocardcolor:'#292a2f',//个人信息界面的个人信息卡片背景色
      personalInfocardcolor:'rgba(255,255,255,0.1)',//个人信息界面的个人信息卡片背景色 
      personalInfocardcolor2: 'rgba(255, 255, 255,0.1)',//个人信息界面的个人信息卡片背景色
      tableheaderbgcolor: '#434343',//表头背景色
      textcolordanger: '#FF6B5A',//智能对话里 AI回答气泡里error-content区域的文字颜色

    },
    preview: {
      background: 'linear-gradient(135deg, #111318 0%, #232938 100%)',
      surface: '#202126',
      sidebar: '#101113',
      accent: '#60a5fa',
      text: '#f3f4f6',
    },
  },
  {
    id: 'scene-tech-blue',
    name: '科技蓝',
    type: 'Purple',
    description: '清爽蓝紫科技界面',
    category: 'scene',
    defaultOpacity: createThemeOpacity(10, 35, 30, 0),
    palette: {
      mode: 'light',
      bgPrimary: '#294377',
      kanbanCardBackground: 'rgba(255, 255, 255, 0.1)',//调用看板界面的卡片背景颜色
      inputPrimary: '#fff',
      //bgSidebar: '#1e3568',
      bgSidebar: '#000000',  //主菜单栏背景透明度基础色
      bgTitlebar: '#23417d',
      bgTabbar: '#27488a',
      bgSubmenu: '#32518e',
      bgHover: 'rgba(255, 255, 255, 0.08)',
      modelBgHover: 'rgba(255, 255, 255, 0.08)',//模型管理界面左侧供应商列表选中悬浮背景色
      bgActive: 'rgba(255, 255, 255, 0.86)',
      textPrimary: '#ffffff',
      textSecondary: '#ffffff',
      textThree: '#a9b7c6',//智能对话AI回答区域的代码注释文字颜色
      // borderColor: '#5f7fc7',
      // borderColor: '#81a2f0ff',
      borderColor: 'rgba(255, 255, 255, 0.1)',
      // colorPrimary: '#4167eb',
      colorPrimary: '#0089D7',
      colormyWallet:'#0089D7',//我的钱包界面颜色
      colormyWalletBtnHover: '#0079bd',
      colorbackgroundone: '#ecebe4',//卡片背景渐变颜色1
      colorbackgroundtow: '#adcadb',//卡片背景渐变颜色2
      colorbackgroundthree: '#82c3e9',//卡片背景渐变颜色3

      BtnBackgroundColor: '#0089D7',//按钮背景颜色
      BtnHoverBackgroundColor: '#4096FF',//鼠标悬浮按钮背景颜色 ，
      MeunWenzhiActiveColor: ' #ffffff',//菜单选中文字颜色
      colorSuccess: '#22c55e',
      colorWarning: '#d97706',
      colorDanger: '#dc2626',
      appBackground: 'linear-gradient(135deg, #213569 0%, #2347b5 35%, #263865 100%)',
      appBackgroundImage: PurpleBackgroundImage,
      modalBg: '#304982',
      cardBg: '#35508d',
      //questionBubble: '#5e83daff', 
      questionBubble: 'rgba(0, 0, 0, 0.1)',
      xlBoxText: '#000',
      // mainChatBackground: 'rgba(0, 0, 0, 0.25)', //智能对话 聊天区背景颜色 'rgba(0, 0, 0, 0.25)'
      // asideChatHistory: 'rgba(0, 0, 0, 0.2)', //智能对话 聊天区背景颜色
      // tabBarBackground:'rgba(0, 0, 0, 0.2)', //智能对话 会话历史背景颜色
      // aiAnswerBackground: 'rgba(0, 0, 0, 0.1)',  //智能对话 回答背景颜色
      // chatDividerColor: 'rgba(255, 255, 255, 0.05)', //智能对话 分割线颜色
      mainChatBackground: 'rgba(0, 0, 0, 0)', //智能对话 聊天区背景颜色 'rgba(0, 0, 0, 0.25)'
      asideChatHistory: 'rgba(0, 0, 0, 0)', //智能对话 聊天区背景颜色
      tabBarBackground: 'rgba(0, 0, 0, 0)', //智能对话 会话历史背景颜色
      aiAnswerBackground: 'rgba(0, 0, 0, 0.1)',  //智能对话 回答背景颜色
      chatDividerColor: 'rgba(255, 255, 255, 0.05)', //智能对话 分割线颜色
      adBannerColor: 'rgba(0, 0, 0, 0)',//顶部广告背景
      convListHoverColor: 'rgba(255, 255, 255, 0.05)',//智能对话 历史会话item鼠标悬浮背
      convListHoverActiveColor: 'rgba(255, 255, 255, 0.05)',//智能对话 历史会话item选中背景色
      convListHoverActiveBorderColor: 'rgba(255, 255, 255, 0.2)',//智能对话 历史会话item鼠标选中边框景色
      chatInputSkillChip: 'rgba(0, 0, 0, 0)',//skill-chip（写周末计划 ，智能对话消息输入框 背景颜色）
      mainMeunActiveBackground: '#0089D7', //主菜单选中背景色
      mainMeunHoverBackground: '#6393FF',//主菜单鼠标悬浮Item背景色
      BackgroundColor: 'rgba(255, 255, 255, 0.1)',//知识库列表界面卡片背景颜色
      TextColorLightBlack: '#ffffff',//转为文档编辑的ai回答界面 文字颜色随系统改变
      TabActiveHover: 'rgba(255, 255, 255,0.1)',//应用配置，API接入渠道， 未选中背景
      // ThemeDialogColor: '#294377',//主题 弹窗，下拉框背景色 57, 81, 135
      ThemeDialogColor:  'rgb(59, 86, 148)',
       cancelbtncolor: 'rgb(52, 81, 146)',
      cancelbtnhovercolor: '#3d6eb6',
      DropdownHoverColor: 'rgba(255, 255, 255,0.1)',//下拉框 鼠标悬浮色
      webkitScrollbarTHumb: 'rgba(255, 255, 255,0.4)',//ai回答区域滚动条颜色
      CardBackgroundTmd: 'rgba(255, 255, 255,0.1)',//带10%透明度的背景色
      SpecialTextColor:'#f5f541',//特殊文字颜色
      infoambetextcolor:'#d97706',
      SpecialTiptextColor:'#f5f541',//特殊提示文字颜色
      VibeCodeSpecialText:'#fff',//情景主题下为白色文字，其余为主题色
      treemenuSearchingColor:  'rgba(255, 255, 255, 0.45)',//菜单搜索模式下的背景色
      // personalInfocardcolor:'#6c95fd',//个人信息界面的个人信息卡片背景色
      personalInfocardcolor:'rgba(255, 255, 255,0.1)',//个人信息界面的个人信息卡片背景色 \
      personalInfocardcolor2: 'rgba(255, 255, 255,0.1)',//个人信息界面的个人信息卡片背景色
      tableheaderbgcolor: '#739bd6',//表头背景色
      textcolordanger: '#FF6B5A',//智能对话里 AI回答气泡里error-content区域的文字颜色
    },
    preview: {
      background: 'linear-gradient(135deg, #1e3568 0%, #4167eb 100%)',
      surface: '#294377',
      sidebar: '#1e3568',
      accent: '#4167eb',
      text: '#ffffff',
    },
  },
  {
    id: 'scene-forest-green',
    name: '森系绿',
    type: 'Green',
    description: '自然绿色与柔和浅底',
    category: 'scene',
    defaultOpacity: createThemeOpacity(10, 35, 30, 0),
    palette: {
      mode: 'light',
      bgPrimary: '#2b4738',
      kanbanCardBackground: 'rgba(255, 255, 255, 0.1)',//调用看板界面的卡片背景颜色
      inputPrimary: '#2b4738',
      bgSidebar: '#000000',  //主菜单栏背景透明度基础色 //'#1d3628',
      bgTitlebar: '#244231',
      bgTabbar: '#284838',
      bgSubmenu: '#315242',
      bgHover: 'rgba(255, 255, 255, 0.08)',
      modelBgHover: 'rgba(255, 255, 255, 0.08)',//模型管理界面左侧供应商列表选中悬浮背景色
      bgActive: 'rgba(255, 255, 255, 0.86)',
      textPrimary: '#ffffff',
      textSecondary: '#ffffff', 
      textThree: '#a9b7c6',//智能对话AI回答区域的代码注释文字颜色
      // borderColor: '#5cc1ab',
      borderColor: 'rgba(255, 255, 255, 0.1)',
      colorPrimary: '#2F9E44',
      colormyWallet:'#2F9E44',//我的钱包界面颜色
      colormyWalletBtnHover: '#298b3c',
      colorbackgroundone: '#ccdbcf',//卡片背景渐变颜色1
      colorbackgroundtow: '#76a17f',//卡片背景渐变颜色2
      colorbackgroundthree: '#5faa6e',//卡片背景渐变颜色3

      BtnBackgroundColor: '#2F9E44',//按钮背景颜色
      BtnHoverBackgroundColor: '#40C057',//鼠标悬浮按钮背景颜色 ，主菜单选中背景颜色
      MeunWenzhiActiveColor: ' #ffffff',//菜单选中文字颜色
      colorSuccess: '#22c55e',
      colorWarning: '#d97706',
      colorDanger: '#dc2626',
      appBackground: 'linear-gradient(135deg, #fbfff8 0%, #dff4dc 100%)',
      appBackgroundImage: GreenBackgroundImage,
      modalBg: '#305141',
      cardBg: '#355947',
      questionBubble: 'rgba(0, 0, 0, 0.1)',
      xlBoxText: '#000',
      mainChatBackground: 'rgba(0, 0, 0, 0.25)', //智能对话 聊天区背景颜色
      asideChatHistory: 'rgba(0, 0, 0, 0.2)', //智能对话 聊天区背景颜色
      tabBarBackground: 'rgba(0, 0, 0, 0.2)', //智能对话 会话历史背景颜色
      aiAnswerBackground: 'rgba(0, 0, 0, 0.1)',  //智能对话 回答背景颜色
      chatDividerColor: 'rgba(255, 255, 255, 0.05)', //智能对话 分割线颜色
      adBannerColor: 'rgba(0, 0, 0, 0)',//顶部广告背景
      convListHoverColor: 'rgba(255, 255, 255, 0.05)',//智能对话 历史会话item鼠标悬浮背
      convListHoverActiveColor: 'rgba(255, 255, 255, 0.05)',//智能对话 历史会话item选中背景色
      convListHoverActiveBorderColor: 'rgba(255, 255, 255, 0.2)',//智能对话 历史会话item鼠标选中边框景色
      chatInputSkillChip: 'rgba(0, 0, 0, 0)',//skill-chip（写周末计划 ，智能对话消息输入框 背景颜色）
      mainMeunActiveBackground: '#16a34a', //主菜单选中背景色
      mainMeunHoverBackground: '#5CC2AC',//主菜单鼠标悬浮Item背景色
      BackgroundColor: 'rgba(255, 255, 255, 0.1)',//知识库列表界面卡片背景颜色
      TextColorLightBlack: '#ffffff',//转为文档编辑的ai回答界面 文字颜色随系统改变
      TabActiveHover: 'rgba(255, 255, 255,0.1)',//应用配置，API接入渠道， 未选中背景
      // ThemeDialogColor: '#2b4738',//主题 弹窗，下拉框背景色
      ThemeDialogColor:  'rgb(60, 116, 98)',
            cancelbtncolor: 'rgb(53, 112, 93)',
      cancelbtnhovercolor: 'rgb(46, 107, 89)',
      DropdownHoverColor: 'rgba(255, 255, 255,0.1)',//下拉框 鼠标悬浮色
      webkitScrollbarTHumb: 'rgba(255, 255, 255,0.4)',//ai回答区域滚动条颜色
      CardBackgroundTmd: 'rgba(255, 255, 255,0.1)',//带10%透明度的背景色
      SpecialTextColor:'#f5f541',//特殊文字颜色
      infoambetextcolor:'#d97706',
      SpecialTiptextColor:'#f5f541',//特殊提示文字颜色
      VibeCodeSpecialText:'#fff',//情景主题下为白色文字，其余为主题色
      treemenuSearchingColor:  'rgba(255, 255, 255, 0.45)',//菜单搜索模式下的背景色
      // personalInfocardcolor:'#37c1a3',//个人信息界面的个人信息卡片背景色
      personalInfocardcolor:'rgba(255, 255, 255,0.1)',//个人信息界面的个人信息卡片背景色 \
      personalInfocardcolor2: 'rgba(255, 255, 255,0.1)',//个人信息界面的个人信息卡片背景色
      tableheaderbgcolor: '#6eb4a3',//表头背景色
      textcolordanger: '#FF6B5A',//智能对话里 AI回答气泡里error-content区域的文字颜色
    },
    preview: {
      background: 'linear-gradient(135deg, #1d3628 0%, #16a34a 100%)',
      surface: '#2b4738',
      sidebar: '#1d3628',
      accent: '#16a34a',
      text: '#ffffff',
    },
  },
  {
    id: 'scene-warm-sun',
    name: '暖光黄',
    type: 'Orange',
    description: '温暖米黄与橙色强调',
    category: 'scene',
    defaultOpacity: createThemeOpacity(10, 35, 30, 0),
    palette: {
      mode: 'light',
      bgPrimary: '#654621',
      kanbanCardBackground: 'rgba(255, 255, 255, 0.1)',//调用看板界面的卡片背景颜色
      inputPrimary: '#654621',
      bgSidebar: '#4f3515',
      bgTitlebar: '#5b3d19',
      bgTabbar: '#67461b',
      bgSubmenu: '#745128',
      bgHover: 'rgba(255, 255, 255, 0.08)',
      modelBgHover: 'rgba(255, 255, 255, 0.08)',//模型管理界面左侧供应商列表选中悬浮背景色
      bgActive: 'rgba(255, 255, 255, 0.86)',
      textPrimary: '#ffffff',
      textSecondary: '#ffffff',
      textThree: '#a9b7c6',//智能对话AI回答区域的代码注释文字颜色
      // borderColor: '#f9c58e',
      borderColor: 'rgba(255, 255, 255, 0.1)',
      colorPrimary: '#FAAD14',
      colormyWallet:'#FAAD14',//我的钱包界面颜色
      colormyWalletBtnHover: '#dc9812',
      colorbackgroundone: '#fffbeb',//卡片背景渐变颜色1
      colorbackgroundtow: '#fef3c7',//卡片背景渐变颜色2
      colorbackgroundthree: '#fde68a',//卡片背景渐变颜色3

      BtnBackgroundColor: '#FAAD14',//按钮背景颜色
      BtnHoverBackgroundColor: '#d97706',//鼠标悬浮按钮背景颜色 ，主菜单选中背景颜色
      MeunWenzhiActiveColor: ' #ffffff',//菜单选中文字颜色
      colorSuccess: '#16a34a',
      colorWarning: '#f59e0b',
      colorDanger: '#dc2626',
      appBackground: 'linear-gradient(135deg, #fffaf0 0%, #ffe0a6 100%)',
      appBackgroundImage: OrangeBackgroundImage,
      modalBg: '#6d4c24',
      cardBg: '#76542a',
      questionBubble: 'rgba(0, 0, 0, 0.1)',
      xlBoxText: '#000',
      mainChatBackground: 'rgba(0, 0, 0, 0.25)', //智能对话 聊天区背景颜色
      asideChatHistory: 'rgba(0, 0, 0, 0.2)', //智能对话 聊天区背景颜色
      tabBarBackground: 'rgba(0, 0, 0, 0.2)', //智能对话 会话历史背景颜色
      aiAnswerBackground: 'rgba(0, 0, 0, 0.1)',  //智能对话 回答背景颜色
      chatDividerColor: 'rgba(255, 255, 255, 0.05)', //智能对话 分割线颜色
      adBannerColor: 'rgba(0, 0, 0, 0)',//顶部广告背景
      convListHoverColor: 'rgba(255, 255, 255, 0.05)',//智能对话 历史会话item鼠标悬浮背
      convListHoverActiveColor: 'rgba(255, 255, 255, 0.05)',//智能对话 历史会话item选中背景色
      convListHoverActiveBorderColor: 'rgba(255, 255, 255, 0.2)',//智能对话 历史会话item鼠标选中边框景色
      chatInputSkillChip: 'rgba(0, 0, 0, 0)',//skill-chip（写周末计划 ，智能对话消息输入框 背景颜色）
      mainMeunActiveBackground: '#d97706', //主菜单选中背景色
      mainMeunHoverBackground: '#FFCC8F',//主菜单鼠标悬浮Item背景色
      BackgroundColor: 'rgba(255, 255, 255, 0.1)',//知识库列表界面卡片背景颜色
      TextColorLightBlack: '#ffffff',//转为文档编辑的ai回答界面 文字颜色随系统改变
      TabActiveHover: 'rgba(255, 255, 255,0.1)',//应用配置，API接入渠道， 未选中背景
      // ThemeDialogColor: '#654621',//主题 弹窗，下拉框背景色
      ThemeDialogColor: 'rgb(163, 126, 64)',
      cancelbtncolor: 'rgb(150, 114, 51)',
      cancelbtnhovercolor: 'rgb(151, 115, 54)',
      DropdownHoverColor: 'rgba(255, 255, 255,0.1)',//下拉框 鼠标悬浮色
      webkitScrollbarTHumb: 'rgba(255, 255, 255,0.4)',//ai回答区域滚动条颜色
      CardBackgroundTmd: 'rgba(255, 255, 255,0.1)',//带10%透明度的背景色
      SpecialTextColor:'#f5f541',//特殊文字颜色
      infoambetextcolor:'#f5f541',      
      SpecialTiptextColor:'#f5f541',//特殊提示文字颜色
      VibeCodeSpecialText:'#fff',//情景主题下为白色文字，其余为主题色
      treemenuSearchingColor:  'rgba(255, 255, 255, 0.45)',//菜单搜索模式下的背景色
      // personalInfocardcolor:'#d7a658',//个人信息界面的个人信息卡片背景色
      personalInfocardcolor:'rgba(255, 255, 255,0.1)',//个人信息界面的个人信息卡片背景色 \
      personalInfocardcolor2: 'rgba(255, 255, 255,0.1)',//个人信息界面的个人信息卡片背景色
      tableheaderbgcolor: '#dbba92',//表头背景色
      textcolordanger: '#fff600',//智能对话里 AI回答气泡里error-content区域的文字颜色
    },
    preview: {
      background: 'linear-gradient(135deg, #4f3515 0%, #d97706 100%)',
      surface: '#654621',
      sidebar: '#4f3515',
      accent: '#d97706',
      text: '#ffffff',
    },
  },
  {
    id: 'scene-aurora-purple',
    name: '晴空蓝',
    type: 'Blue',
    description: '明净天蓝与轻盈浅色界面',
    category: 'scene',
    defaultOpacity: createThemeOpacity(10, 35, 30, 0),
    palette: {
      mode: 'light',
      bgPrimary: '#28445a',
      kanbanCardBackground: 'rgba(255, 255, 255, 0.1)',//调用看板界面的卡片背景颜色
      inputPrimary: '#28445a',
      bgSidebar: '#1c3347',
      bgTitlebar: '#224058',
      bgTabbar: '#2a4c69',
      bgSubmenu: '#335975',
      bgHover: 'rgba(255, 255, 255, 0.08)',
      modelBgHover: 'rgba(255, 255, 255, 0.08)',//模型管理界面左侧供应商列表选中悬浮背景色
      bgActive: 'rgba(255, 255, 255, 0.86)',
      textPrimary: '#ffffff',
      textSecondary: '#ffffff',
      textThree: '#a9b7c6',//智能对话AI回答区域的代码注释文字颜色
      // borderColor: '#80cff8', //智能对话界面左侧菜单和对话区域分割线颜色
      borderColor: 'rgba(255, 255, 255, 0.1)',
      colorPrimary: '#38BDF8',
      colormyWallet:'#38BDF8',//我的钱包界面颜色
      colormyWalletBtnHover: '#31a6da',
      colorbackgroundone: '#becfd6',//卡片背景渐变颜色1
      colorbackgroundtow: '#97b5c2',//卡片背景渐变颜色2
      colorbackgroundthree: '#81c2dd',//卡片背景渐变颜色3

      BtnBackgroundColor: '#38BDF8',//按钮背景颜色
      BtnHoverBackgroundColor: '#7DD3FC',//鼠标悬浮按钮背景颜色 ，主菜单选中背景颜色
      MeunWenzhiActiveColor: ' #ffffff',//菜单选中文字颜色
      colorSuccess: '#16a34a',
      colorWarning: '#d97706',
      colorDanger: '#dc2626',
      appBackground: 'linear-gradient(135deg, #f8fdff 0%, #cdefff 100%)',
      appBackgroundImage: BlueBackgroundImage,
      modalBg: '#305066',
      cardBg: '#365c74',
      questionBubble: 'rgba(0, 0, 0, 0.1)',
      xlBoxText: '#000',
      mainChatBackground: 'rgba(0, 0, 0, 0.25)', //智能对话 聊天区背景颜色
      asideChatHistory: 'rgba(0, 0, 0, 0.2)', //智能对话 聊天区背景颜色
      tabBarBackground: 'rgba(0, 0, 0, 0.2)', //智能对话 会话历史背景颜色
      aiAnswerBackground: 'rgba(0, 0, 0, 0.1)',  //智能对话 回答背景颜色
      chatDividerColor: 'rgba(255, 255, 255, 0.05)', //智能对话 分割线颜色
      adBannerColor: 'rgba(0, 0, 0, 0)',//顶部广告背景
      convListHoverColor: 'rgba(255, 255, 255, 0.05)',//智能对话 历史会话item鼠标悬浮背
      convListHoverActiveColor: 'rgba(255, 255, 255, 0.05)',//智能对话 历史会话item选中背景色
      convListHoverActiveBorderColor: 'rgba(255, 255, 255, 0.2)',//智能对话 历史会话item鼠标选中边框景色
      chatInputSkillChip: 'rgba(0, 0, 0, 0)',//skill-chip（写周末计划 ，智能对话消息输入框 背景颜色）
      mainMeunActiveBackground: '#0ea5e9', //主菜单选中背景色
      mainMeunHoverBackground: '#2CAEF7',//主菜单鼠标悬浮Item背景色
      BackgroundColor: 'rgba(255, 255, 255, 0.1)',//知识库列表界面卡片背景颜色
      TextColorLightBlack: '#ffffff',//转为文档编辑的ai回答界面 文字颜色随系统改变
      TabActiveHover: 'rgba(255, 255, 255,0.1)',//应用配置，API接入渠道， 未选中背景
      // ThemeDialogColor: '#28445a',//主题 弹窗，下拉框背景色
      ThemeDialogColor: 'rgb(74, 128, 156)',
      cancelbtncolor: 'rgb(68, 116, 139)',
      cancelbtnhovercolor: 'rgb(62, 118, 146)',
      DropdownHoverColor: 'rgba(255, 255, 255,0.1)',//下拉框 鼠标悬浮色
      webkitScrollbarTHumb: 'rgba(255, 255, 255,0.4)',//ai回答区域滚动条颜色
      CardBackgroundTmd: 'rgba(255, 255, 255,0.1)',//带10%透明度的背景色
      SpecialTextColor:'#f5f541',//特殊文字颜色
      infoambetextcolor:'#d97706',
      SpecialTiptextColor:'#f5f541',//特殊提示文字颜色
      VibeCodeSpecialText:'#fff',//情景主题下为白色文字，其余为主题色
      treemenuSearchingColor:  'rgba(255, 255, 255, 0.45)',//菜单搜索模式下的背景色
      // personalInfocardcolor:'#51c5f7',//个人信息界面的个人信息卡片背景色
      personalInfocardcolor:'rgba(255, 255, 255,0.1)',//个人信息界面的个人信息卡片背景色 \
      personalInfocardcolor2: 'rgba(255, 255, 255,0.1)',//个人信息界面的个人信息卡片背景色
      tableheaderbgcolor: '#73b4d3',//表头背景色
      textcolordanger: '#ffb260',//智能对话里 AI回答气泡里error-content区域的文字颜色
    },
    preview: {
      background: 'linear-gradient(135deg, #1c3347 0%, #0ea5e9 100%)',
      surface: '#28445a',
      sidebar: '#1c3347',
      accent: '#0ea5e9',
      text: '#ffffff',
    },
  },
  {
    id: 'scene-rose-dusk',
    name: '暮色红',
    type: 'Red',
    description: '柔和暮色与玫瑰红强调',
    category: 'scene',
    defaultOpacity: createThemeOpacity(10, 35, 30, 0),
    palette: {
      mode: 'light',
      bgPrimary: '#5d2d39',
      kanbanCardBackground: 'rgba(255, 255, 255, 0.1)',//调用看板界面的卡片背景颜色
      inputPrimary: '#5d2d39',
      bgSidebar: '#49202b',
      bgTitlebar: '#552631',
      bgTabbar: '#643140',
      bgSubmenu: '#734355',
      bgHover: 'rgba(255, 255, 255, 0.08)',
      modelBgHover: 'rgba(255, 255, 255, 0.08)',//模型管理界面左侧供应商列表选中悬浮背景色
      bgActive: 'rgba(255, 255, 255, 0.86)',
      textPrimary: '#ffffff',
      textSecondary: '#ffffff',
      textThree: '#a9b7c6',//智能对话AI回答区域的代码注释文字颜色
      // borderColor: '#e18c7f',
      borderColor: 'rgba(255, 255, 255, 0.1)',
      colorPrimary: '#E03131',
      colormyWallet:'#E03131',//我的钱包界面颜色
      colormyWalletBtnHover: '#c52b2b',
      colorbackgroundone: '#e2d8d8',//卡片背景渐变颜色1
      colorbackgroundtow: '#d1a4a4',//卡片背景渐变颜色2
      colorbackgroundthree: '#cf7e7e',//卡片背景渐变颜色3

      BtnBackgroundColor: '#E03131',//按钮背景颜色
      BtnHoverBackgroundColor: '#E03131',//鼠标悬浮按钮背景颜色 ，主菜单选中背景颜色
      MeunWenzhiActiveColor: ' #ffffff',//菜单选中文字颜色
      colorSuccess: '#16a34a',
      colorWarning: '#f59e0b',
      colorDanger: '#dc2626',
      appBackground: 'linear-gradient(135deg, #fffafa 0%, #ffd6dc 100%)',
      appBackgroundImage: RedBackgroundImage,
      modalBg: '#653340',
      cardBg: '#71404d',
      questionBubble: 'rgba(0, 0, 0, 0.1)',
      xlBoxText: '#000',
      mainChatBackground: 'rgba(0, 0, 0, 0.25)', //智能对话 聊天区背景颜色
      asideChatHistory: 'rgba(0, 0, 0, 0.2)', //智能对话 聊天区背景颜色
      tabBarBackground: 'rgba(0, 0, 0, 0.2)', //智能对话 会话历史背景颜色
      aiAnswerBackground: 'rgba(0, 0, 0, 0.1)',  //智能对话 回答背景颜色
      chatDividerColor: 'rgba(255, 255, 255, 0.05)', //智能对话 分割线颜色
      adBannerColor: 'rgba(0, 0, 0, 0)',//顶部广告背景
      convListHoverColor: 'rgba(255, 255, 255, 0.05)',//智能对话 历史会话item鼠标悬浮背
      convListHoverActiveColor: 'rgba(255, 255, 255, 0.05)',//智能对话 历史会话item选中背景色
      convListHoverActiveBorderColor: 'rgba(255, 255, 255, 0.2)',//智能对话 历史会话item鼠标选中边框景色
      chatInputSkillChip: 'rgba(0, 0, 0, 0)',//skill-chip（写周末计划 ，智能对话消息输入框 背景颜色）
      mainMeunActiveBackground: '#e11d48', //主菜单选中背景色
      mainMeunHoverBackground: '#F04D49',//主菜单鼠标悬浮Item背景色
      BackgroundColor: 'rgba(255, 255, 255, 0.1)',//知识库列表界面卡片背景颜色
      TextColorLightBlack: '#ffffff',//转为文档编辑的ai回答界面 文字颜色随系统改变
      TabActiveHover: 'rgba(255, 255, 255,0.1)',//应用配置，API接入渠道， 未选中背景
      // ThemeDialogColor: '#5d2d39',//主题 弹窗，下拉框背景色
      ThemeDialogColor: 'rgb(150, 80, 85)',
      cancelbtncolor: 'rgb(145, 76, 81)',
      cancelbtnhovercolor: 'rgb(143, 76, 81)',
      DropdownHoverColor: 'rgba(255, 255, 255,0.1)',//下拉框 鼠标悬浮色
      webkitScrollbarTHumb: 'rgba(255, 255, 255,0.4)',//ai回答区域滚动条颜色
      CardBackgroundTmd: 'rgba(255, 255, 255,0.1)',//带10%透明度的背景色
      SpecialTextColor:'#f5f541',//特殊文字颜色
      infoambetextcolor:'#d97706',
      SpecialTiptextColor:'#f5f541',//特殊提示文字颜色
      VibeCodeSpecialText:'#fff',//情景主题下为白色文字，其余为主题色
      treemenuSearchingColor:  'rgba(255, 255, 255, 0.45)',//菜单搜索模式下的背景色
      // personalInfocardcolor:'#fd7066',//个人信息界面的个人信息卡片背景色
      personalInfocardcolor:'rgba(255, 255, 255,0.1)',//个人信息界面的个人信息卡片背景色 \
      personalInfocardcolor2: 'rgba(255, 255, 255,0.1)',//个人信息界面的个人信息卡片背景色
     tableheaderbgcolor: '#ca766f',//表头背景色
     textcolordanger: '#ffb260',//智能对话里 AI回答气泡里error-content区域的文字颜色
    },
    preview: {
      background: 'linear-gradient(135deg, #49202b 0%, #e11d48 100%)',
      surface: '#5d2d39',
      sidebar: '#49202b',
      accent: '#e11d48',
      text: '#ffffff',
    },
  },
  {
    id: 'scene-graphite-cyan',
    name: '雾灰调',
    type: 'Grey',
    description: '中性灰阶与沉稳低饱和界面',
    category: 'scene',
    defaultOpacity: createThemeOpacity(10, 35, 30, 0),
    palette: {
      mode: 'light',
      bgPrimary: '#f6f7f9',
      kanbanCardBackground: '#f6f7f9',//调用看板界面的卡片背景颜色
      inputPrimary: '#f6f7f9',
      bgSidebar: '#e8ebef',
      bgTitlebar: '#fafafa',
      bgTabbar: '#eef0f3',
      bgSubmenu: '#ffffff',
      bgHover: 'rgba(71, 85, 105, 0.08)',
      modelBgHover: 'rgba(255, 255, 255, 0.08)',//模型管理界面左侧供应商列表选中悬浮背景色
      bgActive: 'rgba(100, 116, 139, 0.15)',
      textPrimary: '#26313f',
      textSecondary: '#687486',
      textThree: '#a9b7c6',//智能对话AI回答区域的代码注释文字颜色
      borderColor:  'rgba(0, 0, 0, 0.1)',
      colorPrimary: '#8F98A5',
      colormyWallet:'#8F98A5',//我的钱包界面颜色
      colormyWalletBtnHover: '#7e8691',
      colorbackgroundone: '#f6f7fa',//卡片背景渐变颜色1
      colorbackgroundtow: '#dee1e6',//卡片背景渐变颜色2
      colorbackgroundthree: '#dde2e9',//卡片背景渐变颜色3

      BtnBackgroundColor: '#8F98A5',//按钮背景颜色
      BtnHoverBackgroundColor: '#8F98A5',//鼠标悬浮按钮背景颜色 ，主菜单选中背景颜色
      MeunWenzhiActiveColor: ' #ffffff',//菜单选中文字颜色
      colorSuccess: '#16a34a',
      colorWarning: '#d97706',
      colorDanger: '#dc2626',
      appBackground: 'linear-gradient(135deg, #fafafa 0%, #d7dce3 100%)',
      appBackgroundImage: GreyBackgroundImage,
      modalBg: '#fafafa',
      cardBg: '#ffffff',
      questionBubble: 'rgba(0, 0, 0, 0.1)',
      xlBoxText: '#000',
      mainChatBackground: 'rgba(0, 0, 0, 0)', //智能对话 聊天区背景颜色
      asideChatHistory: 'rgba(0, 0, 0, 0)', //智能对话 聊天区背景颜色
      tabBarBackground: 'rgba(0, 0, 0, 0)', //智能对话 会话历史背景颜色
      aiAnswerBackground: 'rgba(255, 255, 255, 1)',  //智能对话 回答背景颜色
      chatDividerColor: 'rgba(0, 0, 0, 0.05)', //智能对话 分割线颜色（白色20%透明度）
      adBannerColor: 'rgba(0, 0, 0, 0)',//顶部广告背景
      convListHoverColor: '#BEBEBE',//智能对话 历史会话item鼠标悬浮背
      convListHoverActiveColor: 'rgba(255, 255, 255, 0.05)',//智能对话 历史会话item选中背景色
      convListHoverActiveBorderColor: 'rgba(0, 0, 0, 0.2)',//智能对话 历史会话item鼠标选中边框景色
      chatInputSkillChip: 'rgba(0, 0, 0, 0)',//skill-chip（写周末计划 ，智能对话消息输入框 背景颜色）
      mainMeunActiveBackground: '#8F98A5', //主菜单选中背景色
      mainMeunHoverBackground: '#BEBEBE',//主菜单鼠标悬浮Item背景色
      BackgroundColor: 'rgba(0, 0, 0, 0.05)',//知识库列表界面卡片背景颜色
      TextColorLightBlack: '#000',//转为文档编辑的ai回答界面 文字颜色随系统改变
      TabActiveHover: 'rgba(255, 255, 255,0.1)',//应用配置，API接入渠道， 未选中背景
      ThemeDialogColor: '#f6f7f9',//主题 弹窗，下拉框背景色
      cancelbtncolor: '#f6f7f9',
      cancelbtnhovercolor: '#f6f7f9',
      DropdownHoverColor: 'rgba(0, 0, 0,0.1)',//下拉框 鼠标悬浮色
      webkitScrollbarTHumb: 'rgba(0, 0, 0, 0.10)',//雾灰调滚动条（更浅）
      CardBackgroundTmd: 'rgba(255, 255, 255,0.1)',//带10%透明度的背景色
      SpecialTextColor:'#0089d7',//特殊文字颜色
      infoambetextcolor:'#d97706',
      SpecialTiptextColor:'#b88230',//特殊提示文字颜色
      VibeCodeSpecialText:'#000',//情景主题下为白色文字，其余为主题色
      treemenuSearchingColor:  'rgba(255, 193, 7, 0.15)',//菜单搜索模式下的背景色
    //   personalInfocardcolor:'#e2e2e2',//个人信息界面的个人信息卡片背景色
      personalInfocardcolor:'#ebf1f8',//个人信息界面的个人信息卡片背景色 \
      personalInfocardcolor2: '#edf1f4',//个人信息界面的个人信息卡片背景色
      tableheaderbgcolor: '#ddddde',//表头背景色
      textcolordanger: '#FF6B5A',//智能对话里 AI回答气泡里error-content区域的文字颜色
    },
    preview: {
      background: 'linear-gradient(135deg, #f8fafc 0%, #cbd5e1 100%)',
      surface: '#ffffff',
      sidebar: '#e8ebef',
      accent: '#64748b',
      text: '#26313f',
    },
  },
]

for (const preset of THEME_PRESETS) {
  preset.palette.buttons = createButtonPalette({
    mode: preset.palette.mode,
    surfaceColor: preset.palette.cardBg,
    textColor: preset.palette.textPrimary,
    borderColor: preset.palette.borderColor,
    accentColor: preset.palette.colorPrimary,
    dangerColor: preset.palette.colorDanger,
    warningColor: preset.palette.colorWarning,
  })
  // 晴空蓝、暖光黄主题色亮度偏高，强制按钮文字为白色
  if (preset.id === 'scene-aurora-purple' || preset.id === 'scene-warm-sun') {
    preset.palette.buttons.primary.text = '#ffffff'
    preset.palette.buttons.primary.hoverText = '#ffffff'
  }
}

export const SIMPLE_THEME_IDS: SimpleThemeId[] = ['simple-light', 'simple-dark']
export const SCENE_THEME_IDS: SceneThemeId[] = [
  'scene-tech-blue',
  'scene-forest-green',
  'scene-warm-sun',
  'scene-aurora-purple',
  'scene-rose-dusk',
  'scene-graphite-cyan',
]

const PRESETS_BY_ID = THEME_PRESETS.reduce<Record<string, ThemePreset>>((map, preset) => {
  map[preset.id] = preset
  return map
}, {})

function getPresetOpacity(themeId: SimpleThemeId | SceneThemeId): ThemeOpacity {
  return cloneOpacity((PRESETS_BY_ID[themeId] || PRESETS_BY_ID[DEFAULT_SETTINGS.themeId]).defaultOpacity)
}

function cloneSettings(settings: ThemeSettings): ThemeSettings {
  return {
    themeId: settings.themeId,
    customBaseThemeId: settings.customBaseThemeId,
    opacity: { ...settings.opacity },
  }
}

function clampOpacity(value: unknown): number {
  const numberValue = Number(value)
  if (!Number.isFinite(numberValue)) return 100
  return Math.min(100, Math.max(0, Math.round(numberValue)))
}

function isSimpleThemeId(value: unknown): value is SimpleThemeId {
  return SIMPLE_THEME_IDS.includes(value as SimpleThemeId)
}

function isSceneThemeId(value: unknown): value is SceneThemeId {
  return SCENE_THEME_IDS.includes(value as SceneThemeId)
}

function isThemeId(value: unknown): value is ThemeId {
  return value === 'custom-scene' || isSimpleThemeId(value) || isSceneThemeId(value)
}

function normalizeSettings(value: unknown): ThemeSettings {
  if (!value || typeof value !== 'object') {
    return cloneSettings(DEFAULT_SETTINGS)
  }

  const candidate = value as Partial<ThemeSettings>
  const themeId = isThemeId(candidate.themeId) ? candidate.themeId : DEFAULT_SETTINGS.themeId
  const customBaseThemeId = isSceneThemeId(candidate.customBaseThemeId)
    ? candidate.customBaseThemeId
    : DEFAULT_SETTINGS.customBaseThemeId
  const fallbackOpacity = themeId === 'custom-scene'
    ? DEFAULT_OPACITY
    : getPresetOpacity(themeId)
  const opacitySource = candidate.opacity || fallbackOpacity

  return {
    themeId,
    customBaseThemeId,
    opacity: {
      mainMenu: clampOpacity(opacitySource.mainMenu),
      mainContent: clampOpacity(opacitySource.mainContent),
      subMenu: clampOpacity(opacitySource.subMenu),
      topBar: clampOpacity(opacitySource.topBar),
    },
  }
}

function hexToRgb(hex: string): { r: number; g: number; b: number } | null {
  const normalized = hex.replace('#', '').trim()
  if (![3, 6].includes(normalized.length)) return null
  const full = normalized.length === 3
    ? normalized.split('').map((char) => char + char).join('')
    : normalized
  const value = Number.parseInt(full, 16)
  if (Number.isNaN(value)) return null

  return {
    r: (value >> 16) & 255,
    g: (value >> 8) & 255,
    b: value & 255,
  }
}

function withOpacity(color: string, opacity: number): string {
  const rgb = hexToRgb(color)
  if (!rgb) return color
  return `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, ${clampOpacity(opacity) / 100})`
}

function withCssHexOpacity(value: string, opacity: number): string {
  return value.replace(/#(?:[0-9a-fA-F]{3}|[0-9a-fA-F]{6})\b/g, (match) => {
    const rgb = hexToRgb(match)
    if (!rgb) return match
    return `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, ${opacity})`
  })
}

function resolvePreset(settings: ThemeSettings): ThemePreset {
  const presetId = settings.themeId === 'custom-scene'
    ? settings.customBaseThemeId
    : settings.themeId
  return PRESETS_BY_ID[presetId] || PRESETS_BY_ID[DEFAULT_SETTINGS.themeId]
}

function resolveAppBackground(preset: ThemePreset): string {
  const { palette } = preset
  if (preset.category !== 'scene' || !palette.appBackgroundImage) {
    return palette.appBackground
  }

  // const overlayOpacity = palette.mode === 'dark' ? 0.72 : 0.6
  // const overlay = withCssHexOpacity(palette.appBackground, overlayOpacity)
  // 去掉半透明遮罩${overlay}, 
  return `url("${palette.appBackgroundImage}") center / cover no-repeat`
}

function resolveButtonPalette(palette: ThemePalette): ThemeButtonPalette {
  return palette.buttons || createButtonPalette({
    mode: palette.mode,
    surfaceColor: palette.cardBg,
    textColor: palette.textPrimary,
    borderColor: palette.borderColor,
    accentColor: palette.colorPrimary,
    dangerColor: palette.colorDanger,
    warningColor: palette.colorWarning,
  })
}

function appendButtonVariantTokens(
  target: Record<string, string>,
  name: keyof ThemeButtonPalette,
  variant: ThemeButtonVariant
) {
  target[`--btn-${name}-bg`] = variant.bg
  target[`--btn-${name}-text`] = variant.text
  target[`--btn-${name}-border`] = variant.border
  target[`--btn-${name}-hover-bg`] = variant.hoverBg
  target[`--btn-${name}-hover-text`] = variant.hoverText
  target[`--btn-${name}-hover-border`] = variant.hoverBorder
  target[`--btn-${name}-disabled-bg`] = variant.disabledBg
  target[`--btn-${name}-disabled-text`] = variant.disabledText
  target[`--btn-${name}-disabled-border`] = variant.disabledBorder
}

function elColorVariants(name: string, base: string): Record<string, string> {
  const out: Record<string, string> = { [`--el-color-${name}`]: base }
  for (const pct of [3, 5, 7, 8, 9]) {
    out[`--el-color-${name}-light-${pct}`] = mixColors(base, '#ffffff', pct / 10)
  }
  out[`--el-color-${name}-dark-2`] = mixColors(base, '#000000', 0.2)
  return out
}

function buildThemeTokens(settings: ThemeSettings): Record<string, string> {
  const preset = resolvePreset(settings)
  const { palette } = preset
  const buttons = resolveButtonPalette(palette)
  const tokens: Record<string, string> = {
    '--bg-primary-solid': palette.bgPrimary,
    '--bg-sidebar-solid': palette.bgSidebar,
    '--bg-titlebar-solid': palette.bgTitlebar,
    '--bg-tabbar-solid': palette.bgTabbar,
    '--bg-submenu-solid': palette.bgSubmenu,
    '--bg-primary': withOpacity(palette.bgPrimary, settings.opacity.mainContent),
    '--kanban-card-background': palette.kanbanCardBackground,//调用看板界面的卡片背景颜色

    '--bg-sidebar': withOpacity(palette.bgSidebar, settings.opacity.mainMenu),
    '--bg-titlebar': withOpacity(palette.bgTitlebar, settings.opacity.topBar),
    '--bg-tabbar': withOpacity(palette.bgTabbar, settings.opacity.topBar),
    '--bg-submenu': withOpacity(palette.bgSubmenu, settings.opacity.subMenu),
    '--bg-hover': palette.bgHover,
    '--model-bg-hover': palette.modelBgHover,//模型管理界面左侧供应商列表选中悬浮背景色
    '--bg-active': palette.bgActive,
    '--text-primary': palette.textPrimary,
    '--text-secondary': palette.textSecondary,
    '--text-three':palette.textThree, //智能对话AI回答区域的代码注释文字颜色
    '--border-color': palette.borderColor,
    '--color-primary': palette.colorPrimary,
    '--color-myWalletBtn':palette.colormyWallet,//我的钱包界面按钮颜色
    '--color-myWalletBtn-hover':palette.colormyWalletBtnHover,
    '--color-backgroundone':palette.colorbackgroundone,//卡片背景渐变颜色1
    '--color-backgroundtow':palette.colorbackgroundtow,//卡片背景渐变颜色2
    '--color-backgroundthree':palette.colorbackgroundthree,//卡片背景渐变颜色3
    // '--color-backgroundcard':palette.colorbackgroundcard,

    '--btn-background-color': palette.BtnBackgroundColor,//按钮背景颜色
    '--btn-hover-background-color': palette.BtnHoverBackgroundColor,//鼠标悬浮按钮背景颜色 ，主菜单选中背景颜色
    '--meum-wenzhi-active-color': palette.MeunWenzhiActiveColor,//菜单选中文字颜色

    '--color-success': palette.colorSuccess,
    '--color-warning': palette.colorWarning,
    '--color-danger': palette.colorDanger,
    '--app-background': resolveAppBackground(preset),
    '--theme-modal-bg': palette.modalBg,
    '--theme-card-bg': palette.cardBg,
    '--theme-question-bubble': palette.questionBubble, //问题七百背景颜色
    '--theme-preview-sidebar': preset.preview.sidebar,
    '--theme-preview-accent': preset.preview.accent,
    '--input-primary': palette.inputPrimary, //智能对话 提问无用背景颜色
    '--theme-xlbox-text': palette.xlBoxText, //智能对话 提问无用背景颜色
    '--main-chat-background': palette.mainChatBackground, //智能对话 聊天区背景颜色
    '--aside-Chat-history': palette.asideChatHistory, //智能对话 会话历史背景颜色
    '--tab-bar-background': withOpacity(palette.bgPrimary, settings.opacity.mainContent), //智能对话 tab区域背景颜色（与主体同步）
    '--ai-answer-background': palette.aiAnswerBackground, //智能对话 回答背景颜色
    '--chat-divider-color': palette.chatDividerColor, //智能对话 分割线颜色（白色20%透明度）
    '--ad-banner-color': palette.adBannerColor, //顶部广告背景颜色
    '--conv-list-hover-color': palette.convListHoverColor,//智能对话 历史会话item鼠标悬浮背景色
    '--conv-list-active-color': palette.convListHoverActiveColor,//智能对话 历史会话item鼠标选中背景色
    '--conv-list-active-border-color': palette.convListHoverActiveBorderColor,//智能对话 历史会话item鼠标选中边景色
    //

    '--chat-input-skill-chip': palette.chatInputSkillChip,// skill-chip（写周末计划 ，智能对话消息输入框 背景颜色）
    '--main-meun-active-background': palette.mainMeunActiveBackground, //主菜单选中背景色
    '--main-meun-hover-background': palette.mainMeunHoverBackground, //主菜单鼠标悬浮Item背景色
    '--background-color': palette.BackgroundColor,//知识库列表界面卡片背景颜色
    '--text-color-light-black': palette.TextColorLightBlack,//文字颜色随系统改变
  
    '--tab-active-hover': palette.TabActiveHover,//应用配置，API接入渠道， 未选中背景色
    '--theme-dailog-color': palette.ThemeDialogColor,//主题 弹窗，下拉框背景色
    '--dropdown-hover-color': palette.DropdownHoverColor,//下拉框 鼠标悬浮色
    '--webkit-scrollbar-thumb':palette.webkitScrollbarTHumb,//ai回答区域滚动条颜色
    '--scrollbar-thumb': palette.webkitScrollbarTHumb,//通用滚动条滑块颜色
    '--scrollbar-thumb-hover': palette.mode === 'light'
      ? 'rgba(0, 0, 0, 0.20)'
      : palette.webkitScrollbarTHumb,//浅色主题悬浮略加深，深色主题沿用滑块色
    '--card-background-tmd':palette.CardBackgroundTmd,//带10%透明度的背景色
    '--special-text-color':palette.SpecialTextColor,//特殊文字颜色
    '--info-ambe-text-color':palette.infoambetextcolor,//远程控制提示文字颜色
    '--special-tiptext-color':palette.SpecialTiptextColor,//特殊提示文字颜色
    '--vibe-code-special-text':palette.VibeCodeSpecialText,//情景主题下为白色文字，其余为主题色
    '--treemenu-searching-color':palette.treemenuSearchingColor,//菜单搜索模式下的背景色
    '--personal-Info-card-color':palette.personalInfocardcolor,//个人信息界面的个人信息卡片背景色
    '--personal-Info-card-color2':palette.personalInfocardcolor2,//个人信息界面的个人信息卡片背景色 渐变色
    '--table-header-bg-color':palette.tableheaderbgcolor,//表头背景色
    '--text-color-danger':palette.textcolordanger,//智能对话里 AI回答气泡里error-content区域的文字颜色
    '--cancel-btn-color':palette.cancelbtncolor,
    '--cancel-btn-hover-color':palette.cancelbtnhovercolor,
  
  }

  appendButtonVariantTokens(tokens, 'primary', buttons.primary)
  appendButtonVariantTokens(tokens, 'secondary', buttons.secondary)
  appendButtonVariantTokens(tokens, 'danger', buttons.danger)
  appendButtonVariantTokens(tokens, 'warning', buttons.warning)
  appendButtonVariantTokens(tokens, 'ghost', buttons.ghost)

  // Element Plus 语义色 + 衍生色（light-3/5/7/8/9, dark-2）
  // simple-dark 的 html.dark 选择器优先级更高，EP 暗色 CSS 变量会自动覆盖此处注入
  Object.assign(tokens, elColorVariants('primary', palette.colorPrimary))
  Object.assign(tokens, elColorVariants('success', palette.colorSuccess))
  Object.assign(tokens, elColorVariants('warning', palette.colorWarning))
  Object.assign(tokens, elColorVariants('danger', palette.colorDanger))
  Object.assign(tokens, elColorVariants('error', palette.colorDanger))

  // Element Plus 组件界面变量（背景/文字/边框）
  // 弹窗、下拉浮层统一走 ThemeDialogColor（--theme-dailog-color）
  tokens['--el-bg-color'] = palette.ThemeDialogColor
  tokens['--el-bg-color-overlay'] = palette.ThemeDialogColor
  tokens['--el-bg-color-page'] = palette.bgPrimary
  tokens['--el-fill-color-blank'] = palette.ThemeDialogColor
  tokens['--el-fill-color-light'] = palette.bgHover
  tokens['--el-fill-color'] = palette.bgHover
  tokens['--el-dialog-bg-color'] = palette.ThemeDialogColor
  tokens['--el-mask-color'] = 'rgba(0, 0, 0, 0.5)'
  tokens['--el-text-color-primary'] = palette.textPrimary
  tokens['--el-text-color-regular'] = palette.textPrimary
  tokens['--el-text-color-secondary'] = palette.textSecondary
  tokens['--el-text-color-placeholder'] = palette.textSecondary
  tokens['--el-border-color'] = palette.borderColor
  tokens['--el-border-color-light'] = palette.borderColor
  tokens['--el-border-color-lighter'] = palette.borderColor
  tokens['--el-border-color-extra-light'] = palette.borderColor

  return tokens
}

function escapeThemeTokenValue(value: string): string {
  return value.replace(/\\/g, '\\\\').replace(/;/g, '\\;')
}

function buildRuntimeThemeStyleText(tokens: Record<string, string>): string {
  const declarations = Object.entries(tokens)
    .map(([name, value]) => `  ${name}: ${escapeThemeTokenValue(value)};`)
    .join('\n')

  return `:root {\n${declarations}\n}`
}

function ensureRuntimeThemeStyleElement(): HTMLStyleElement {
  const styleId = 'fastclaw-runtime-theme'
  const existing = document.getElementById(styleId)
  if (existing instanceof HTMLStyleElement) {
    return existing
  }

  const styleElement = document.createElement('style')
  styleElement.id = styleId
  document.head.appendChild(styleElement)
  return styleElement
}

function applySettingsToDocument(settings: ThemeSettings) {
  if (typeof document === 'undefined') return

  const preset = resolvePreset(settings)
  const { palette } = preset
  const root = document.documentElement

  root.dataset.theme = palette.mode
  root.dataset.category = preset.category
  root.dataset.themeId = settings.themeId
  root.classList.toggle('fc-theme-dark', palette.mode === 'dark')
  root.classList.toggle('fc-theme-light', palette.mode === 'light')
  // 保留 Element Plus 暗色类以兼容 el 组件（仅当主题明确为暗色时）
  root.classList.toggle('dark', palette.mode === 'dark')
  root.classList.toggle('light', palette.mode === 'light')

  const styleElement = ensureRuntimeThemeStyleElement()
  styleElement.textContent = buildRuntimeThemeStyleText(buildThemeTokens(settings))
}

function loadStoredSettings(): ThemeSettings {
  if (typeof localStorage === 'undefined') {
    return cloneSettings(DEFAULT_SETTINGS)
  }

  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    return raw ? normalizeSettings(JSON.parse(raw)) : cloneSettings(DEFAULT_SETTINGS)
  } catch {
    return cloneSettings(DEFAULT_SETTINGS)
  }
}

function saveStoredSettings(settings: ThemeSettings) {
  if (typeof localStorage === 'undefined') return
  localStorage.setItem(STORAGE_KEY, JSON.stringify(settings))
}

export const useThemeStore = defineStore('theme', () => {
  const current = ref<ThemeSettings>(cloneSettings(DEFAULT_SETTINGS))
  const draft = ref<ThemeSettings>(cloneSettings(DEFAULT_SETTINGS))
  const snapshot = ref<ThemeSettings | null>(null)
  const initialized = ref(false)

  const simpleThemes = computed(() => THEME_PRESETS.filter((theme) => theme.category === 'simple'))
  const sceneThemes = computed(() => THEME_PRESETS.filter((theme) => theme.category === 'scene'))
  const currentPreset = computed(() => resolvePreset(current.value))
  const draftPreset = computed(() => resolvePreset(draft.value))
  const draftCategory = computed<ThemeCategory>(() => {
    if (draft.value.themeId === 'custom-scene' || isSceneThemeId(draft.value.themeId)) {
      return 'scene'
    }
    return 'simple'
  })

  function initTheme() {
    if (initialized.value) return
    const stored = loadStoredSettings()
    current.value = cloneSettings(stored)
    draft.value = cloneSettings(stored)
    applySettingsToDocument(current.value)
    initialized.value = true
  }

  function beginDraft() {
    initTheme()
    snapshot.value = cloneSettings(current.value)
    draft.value = cloneSettings(current.value)
    applySettingsToDocument(draft.value)
  }

  function selectTheme(themeId: SimpleThemeId | SceneThemeId) {
    draft.value.themeId = themeId
    draft.value.opacity = getPresetOpacity(themeId)
    if (isSceneThemeId(themeId)) {
      draft.value.customBaseThemeId = themeId
    }
    applySettingsToDocument(draft.value)
  }

  function selectCustomTheme() {
    if (isSceneThemeId(draft.value.themeId)) {
      draft.value.customBaseThemeId = draft.value.themeId
    }
    draft.value.themeId = 'custom-scene'
    applySettingsToDocument(draft.value)
  }

  function updateOpacity(key: OpacityKey, value: number) {
    if (isSceneThemeId(draft.value.themeId)) {
      draft.value.customBaseThemeId = draft.value.themeId
    }
    draft.value.themeId = 'custom-scene'
    draft.value.opacity[key] = clampOpacity(value)
    applySettingsToDocument(draft.value)
  }

  function resetDraft() {
    draft.value = cloneSettings(DEFAULT_SETTINGS)
    applySettingsToDocument(draft.value)
  }

  function confirmDraft() {
    current.value = cloneSettings(draft.value)
    snapshot.value = null
    applySettingsToDocument(current.value)
    saveStoredSettings(current.value)
  }

  function cancelDraft() {
    const restored = snapshot.value ? cloneSettings(snapshot.value) : cloneSettings(current.value)
    draft.value = cloneSettings(restored)
    current.value = cloneSettings(restored)
    snapshot.value = null
    applySettingsToDocument(current.value)
  }

  function applyCurrent() {
    applySettingsToDocument(current.value)
  }

  return {
    current,
    draft,
    simpleThemes,
    sceneThemes,
    currentPreset,
    draftPreset,
    draftCategory,
    initTheme,
    beginDraft,
    selectTheme,
    selectCustomTheme,
    updateOpacity,
    resetDraft,
    confirmDraft,
    cancelDraft,
    applyCurrent,
  }
})
