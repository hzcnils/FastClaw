import { describe, expect, it } from 'vitest'
import { mount, shallowMount } from '@vue/test-utils'
import ProviderSidebar from '../components/catalog/ProviderSidebar.vue'
import ProviderConfigPanel from '../components/catalog/ProviderConfigPanel.vue'
import GatewayProviderTabs from '../components/catalog/GatewayProviderTabs.vue'
import ModelToolbar from '../components/catalog/ModelToolbar.vue'
import ModelBatchBar from '../components/catalog/ModelBatchBar.vue'
import ModelCatalogContent from '../components/catalog/ModelCatalogContent.vue'
import type { RawProviderResponse } from '../types/contracts'

const t = (key: string) => key
const provider: RawProviderResponse = {
  id: 'provider-1', name: 'Provider', provider_type: 'openai', base_url: '',
  api_key: 'sk-full-business-key', is_local: false, is_enabled: true,
  provider_code: 'OPENAI', source_category: 'online', extra_config: null,
}
const global = { mocks: { $t: t } }

describe('models catalog presentational components', () => {
  it('forwards sidebar selection without accessing catalog state', async () => {
    const wrapper = shallowMount(ProviderSidebar, {
      props: {
        providers: [provider], selectedProviderId: null, fixedTab: null, search: '',
        moreMenuOpen: false, syncing: false,
      },
      global,
    })
    await wrapper.findAll('.provider-item').at(-1)!.trigger('click')
    expect(wrapper.emitted('selectProvider')?.[0]).toEqual([provider])
  })

  it('only renders built-in provider entries that exist in the database result', () => {
    const wrapper = shallowMount(ProviderSidebar, {
      props: {
        providers: [], selectedProviderId: null, fixedTab: 'all', search: '',
        moreMenuOpen: false,
      },
      global,
    })

    expect(wrapper.text()).not.toContain('汇智网')
    expect(wrapper.text()).not.toContain('私域网')
    expect(wrapper.text()).not.toContain('本电脑')
  })

  it('keeps the complete API key and allows an empty API address', async () => {
    const wrapper = mount(ProviderConfigPanel, {
      props: {
        provider, fixedTab: null, sourceCategoryOptions: [],
        editingBaseUrl: true, editingBaseUrlValue: '',
      },
      global: { ...global, stubs: { ElSelect: true, ElOption: true, ElInput: true } },
    })
    const apiKey = wrapper.find<HTMLInputElement>('.secret-input')
    expect(apiKey.element.value).toBe('sk-full-business-key')
    await apiKey.setValue('sk-another-complete-key')
    await apiKey.trigger('blur')
    expect(wrapper.emitted('apiKeyBlur')?.[0]).toEqual(['sk-another-complete-key'])
    expect(wrapper.props('editingBaseUrlValue')).toBe('')
  })

  it('forwards gateway, toolbar and batch actions', async () => {
    const gateway = shallowMount(GatewayProviderTabs, {
      props: { providers: [], modelValue: null, selectedProvider: provider },
      global,
    })
    await gateway.find('[title="modelManage.editProvider"]').trigger('click')
    expect(gateway.emitted('edit')?.[0]).toEqual([provider])

    const toolbar = shallowMount(ModelToolbar, {
      props: { viewMode: 'card', search: '', canManage: true }, global,
    })
    await toolbar.findAll('.view-toggle-btn')[1].trigger('click')
    expect(toolbar.emitted('update:viewMode')?.[0]).toEqual(['ranking'])

    const batch = shallowMount(ModelBatchBar, {
      props: { selectedCount: 2, allSelected: false }, global,
    })
    await batch.find('.btn-start').trigger('click')
    expect(batch.emitted('toggle')?.[0]).toEqual([true])
  })

  it('renders an empty catalog through the content boundary', () => {
    const wrapper = shallowMount(ModelCatalogContent, {
      props: {
        viewMode: 'card', models: [], providerCode: '', manageMode: false,
        selectedModelIds: new Set<string>(), ratings: {},
      },
      global,
    })
    expect(wrapper.find('.empty-model').exists()).toBe(true)
  })
})
