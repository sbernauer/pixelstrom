<script>
import IconEyeSlash from "@/components/icon/IconEyeSlash.vue";
import IconEye from "@/components/icon/IconEye.vue";

export default {
  name: 'FollowUserButton',
  components: { IconEyeSlash, IconEye },
  props: {
    followsUser:{
      type: Boolean,
      required: true,
    },
    username: {
      type: String,
      required: true,
    },
  },
  emits: ['click'],
  methods: {
    onClick() {
      this.$emit('click', this.username);
    }
  }
};
</script>

<template>
    <button @click="onClick">
      <IconEye v-if="followsUser" class="icon-size icon eye" />
      <IconEyeSlash v-else class="icon-size icon eye-slash"/>
    </button>
</template>

<style scoped>

button {
  border: none;
  background-color: #1e1e1e;
  cursor: pointer;
  display: inline-block;
  text-align: center;
  text-decoration: none;
  transition-duration: 0.4s;
  padding: 2px 5px;
  margin: 2px 1px;
}

.icon-size {
  width: 15px;
  height: 15px;
  align-self: center;
  color: darkgrey;
}

.icon:hover {
  opacity: 70%;
}

.eye {
  color: #02935e;
}
</style>