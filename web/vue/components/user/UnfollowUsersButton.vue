<script>
import IconEyeSlash from "@/components/icon/IconEyeSlash.vue";

export default {
  name: 'UnfollowUsersButton',
  components: { IconEyeSlash },
  props:{
    disabled:{
      type: Boolean,
      default: false,
    }
  },
  emits: ['click'],
  methods: {
    onClick: function () {
      if (this.disabled) {
        return;
      }
      this.$emit('click');
    },
  },
};
</script>

<template>
    <button type="button" :disabled="disabled" @click="onClick">
      <IconEyeSlash  class="icon-size icon"/>
      <span>unfollow all users</span>
    </button>
</template>

<style scoped>

button {
  background-color: #1e1e1e;
  color: darkgrey;
  cursor: pointer;
  display: inline-block;
  text-align: center;
  text-decoration: none;
  transition-duration: 0.4s;
  margin: 2px 1px;
  border: darkgrey solid 1px;
}

.icon-size {
  width: 15px;
  height: 15px;
  padding-right: 5px;
}

button:hover {
  color: #04AA6D;
  border: #04AA6D solid 1px;
}
</style>
