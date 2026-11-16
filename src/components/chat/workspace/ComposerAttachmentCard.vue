<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import type { ChatComposerAttachment } from 'chat-renderer'

const props = defineProps<{
  attachment: ChatComposerAttachment
  imagePreviewList: string[]
  fileIcon?: string
}>()
const emit = defineEmits<{
  remove: [nativeEvent: MouseEvent]
  retry: [nativeEvent: MouseEvent]
}>()
const { t } = useI18n()
const previewIndex = computed(() => {
  const url = props.attachment.previewUrl
  const index = url ? props.imagePreviewList.indexOf(url) : -1
  return index >= 0 ? index : 0
})
function composerAttachmentSizeLabel(attachment: ChatComposerAttachment): string {
  const size = attachment.size;
  if (size === undefined || !Number.isFinite(size)) return '';
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(size < 10 * 1024 ? 1 : 0)} KB`;
  return `${(size / 1024 / 1024).toFixed(1)} MB`;
}

</script>

<template>
          <div
            class="jcr-composer-attachment"
            :class="{ 'jcr-composer-attachment--image': attachment.kind === 'image' }"
            :data-status="attachment.status || 'idle'"
            :title="attachment.error || attachment.name"
            :aria-busy="attachment.status === 'uploading' || attachment.status === 'processing'"
          >
            <template v-if="attachment.kind === 'image'">
              <span class="jcr-composer-attachment__visual" :title="attachment.name">
                <el-image
                  v-if="attachment.previewUrl"
                  class="workspace-composer-att-img"
                  :src="attachment.previewUrl"
                  fit="cover"
                  :preview-src-list="imagePreviewList"
                  :initial-index="previewIndex"
                  :preview-teleported="true"
                  @click.stop
                >
                  <template #error>
                    <div class="workspace-composer-att-img-error">{{ t('chat.uploadFailed') }}</div>
                  </template>
                </el-image>
                <i
                  v-if="attachment.status === 'uploading' || attachment.status === 'processing'"
                  class="jcr-composer-attachment__spinner"
                  role="status"
                  aria-hidden="true"
                />
                <i
                  v-else-if="attachment.status === 'complete'"
                  class="jcr-composer-attachment__status jcr-composer-attachment__status--complete"
                  aria-hidden="true"
                >✓</i>
                <i
                  v-else-if="attachment.status === 'error'"
                  class="jcr-composer-attachment__status jcr-composer-attachment__status--error"
                  aria-hidden="true"
                >!</i>
              </span>
            </template>
            <template v-else>
              <span class="jcr-composer-attachment__visual" :title="attachment.name">
                <img
                  v-if="fileIcon"
                  :src="fileIcon"
                  :alt="attachment.name"
                />
                <i
                  v-if="attachment.status === 'uploading' || attachment.status === 'processing'"
                  class="jcr-composer-attachment__spinner"
                  role="status"
                  aria-hidden="true"
                />
              </span>
              <span class="jcr-composer-attachment__info">
                <strong :title="attachment.name">{{ attachment.name }}</strong>
                <small :class="{ 'jcr-error': attachment.status === 'error' }">
                  {{
                    attachment.status === 'error'
                      ? (attachment.error || t('chat.uploadFailed'))
                      : composerAttachmentSizeLabel(attachment)
                  }}
                </small>
                <i
                  v-if="attachment.status === 'uploading' || attachment.status === 'processing'"
                  class="jcr-composer-attachment__progress"
                  role="progressbar"
                  :aria-valuenow="attachment.progress || 0"
                  aria-valuemin="0"
                  aria-valuemax="100"
                >
                  <i :style="{ width: `${Math.max(0, Math.min(100, attachment.progress || 0))}%` }" />
                </i>
              </span>
              <button
                v-if="attachment.status === 'error'"
                class="jcr-composer-attachment__retry"
                type="button"
                :aria-label="t('common.retry')"
                @click.stop="emit('retry', $event)"
              >↻</button>
            </template>
            <button
              class="jcr-composer-attachment__remove"
              type="button"
              :aria-label="t('common.cancel')"
              @click.stop="emit('remove', $event)"
            >×</button>
          </div>

</template>

<!-- jcr attachment layout is public renderer CSS; host theme overrides remain globally scoped. -->
