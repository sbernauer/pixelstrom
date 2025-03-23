<script setup>
import IconArrowsRight from "@/components/IconArrowsRight.vue";
const props = defineProps({
  isPaintingUser: Boolean,
  user: {
    username: String,
    pixelsPerS: Number,
    averageResponseTimeMs: Number,
  },
});
</script>

<template>
  <IconArrowsRight v-if="isPaintingUser" class="arrow-icon" />
  <span v-else>&nbsp;</span>
  <span>{{ user.username }}</span>
  <span>{{ user.pixelsPerS }}</span>
  <span>{{ user.averageResponseTimeMs }}</span>
</template>

<style scoped>
.arrow-icon{
  width: 15px;
  height: 15px;
  align-self: center;
}
</style>