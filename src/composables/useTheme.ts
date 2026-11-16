
import { ref } from 'vue'

const currentTheme = ref(localStorage.getItem('theme') || 'blue')

export function useTheme() {
  const setTheme = (theme: string) => {
    currentTheme.value = theme
    document.documentElement.setAttribute('data-theme', theme)
    localStorage.setItem('theme', theme)
  }

  return {
    currentTheme,
    setTheme
  }
}
