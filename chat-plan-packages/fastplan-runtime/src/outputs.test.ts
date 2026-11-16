import { describe, expect, it } from 'vitest'
import { formatFastPlanFinalContent } from './finalAnswer'
import { detectFastPlanOutputFiles } from './outputs'

describe('FastPlan generated artifacts', () => {
  it('preserves nested document metadata from a skill output envelope', () => {
    const files = detectFastPlanOutputFiles(JSON.stringify({
      content: 'Task completed. 1 file(s) generated. Download: /api/download/run/output.docx',
      skill_id: 'docx',
      result: { download: { files: [{
        name: 'output.docx', size: 7102,
        mime_type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
        download_url: '/api/download/run/output.docx',
      }] } },
      data: [{ name: 'output.docx', url: '/api/download/run/output.docx' }],
    }))

    expect(files).toEqual([expect.objectContaining({
      name: 'output.docx', url: '/api/download/run/output.docx', size: 7102,
      mime_type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
    })])
  })

  it('replaces only generated-file boilerplate and preserves meaningful answers', () => {
    const files = [{ name: 'output.docx', url: '/api/download/run/output.docx' }]
    expect(formatFastPlanFinalContent(
      'Task completed. 1 file(s) generated. Download: /api/download/run/output.docx', files,
    )).toBe('文档已生成，可在下方下载。')
    expect(formatFastPlanFinalContent('报告已完成，并包含三项关键结论。', files))
      .toBe('报告已完成，并包含三项关键结论。')
  })
})
