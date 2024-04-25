const quill = new Quill('#form_text', {
  modules: {
    toolbar: [
      ['bold', 'italic'],
      ['link', 'blockquote', 'code-block', 'image'],
      [{ list: 'ordered' }, { list: 'bullet' }],
    ],
  },
  theme: 'snow',
});

const form = document.querySelector('form');
form.addEventListener('formdata', (event) => {
  // Append Quill content before submitting
  event.formData.append('form_text', JSON.stringify(quill.getText()));
});
