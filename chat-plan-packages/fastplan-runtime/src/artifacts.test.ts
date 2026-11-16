import { describe, expect, it } from 'vitest'
import { extractFastPlanImages } from './artifacts'

describe('FastPlan image artifacts', () => {
  it('extracts nested MCP images and de-duplicates them', () => {
    const image = { type: 'image', data: 'a'.repeat(40), mimeType: 'image/png' }
    expect(extractFastPlanImages({ output: JSON.stringify([image, image]) })).toEqual([
      { src: `data:image/png;base64,${'a'.repeat(40)}`, mimeType: 'image/png' },
    ])
  })

})
