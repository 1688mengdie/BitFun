import { mergeAttributes, Node } from '@tiptap/core';

export const MarkdownImage = Node.create({
  name: 'markdownImage',
  inline: true,
  group: 'inline',
  atom: true,
  draggable: true,

  addAttributes() {
    return {
      src: {
        default: '',
      },
      alt: {
        default: '',
      },
      title: {
        default: null,
      },
    };
  },

  parseHTML() {
    return [{ tag: 'img[src]' }];
  },

  renderHTML({ HTMLAttributes }) {
    return ['img', mergeAttributes(HTMLAttributes)] as const;
  },
});
