<script>
import IconEyeSlash from '@/components/icon/IconEyeSlash.vue';
import IconEye from '@/components/icon/IconEye.vue';
import IconArrowsRight from '@/components/icon/IconArrowsRight.vue';
import FollowUserButton from '@/components/user/FollowUserButton.vue';

export default {
  name: 'UserDetails',
  components: { FollowUserButton , IconArrowsRight , IconEyeSlash, IconEye },
  props: {
    currentPaintingUser: {
      type: Boolean,
      required: true,
    },
    user: {
      username: String,
      averagePixelsPerRound: Number,
      averageResponseTimeMilliseconds: Number,
    },
    followsUser: {
      type: Boolean,
      required: true,
    },
  },
  emits: ['handleFollowUser'],
  methods: {
    onClick() {
      this.$emit('handleFollowUser', this.user.username);
    }
  },
};
</script>

<template>
  <IconArrowsRight v-if="currentPaintingUser" class="icon-size" />
  <span v-else>&nbsp;</span>
  <span>{{ user.username }}</span>
  <span>{{ user.averagePixelsPerRound.toFixed(0) }}</span>
  <span>{{ user.averageResponseTimeMilliseconds.toFixed(1) }} ms</span>
  <FollowUserButton :username="user.username" :followsUser="followsUser" @click="onClick"/>
</template>

<style scoped>
.icon-size {
  width: 15px;
  height: 15px;
  align-self: center;
}
</style>