# w2z

A form to write structured data into Github. Used for writing notes to static files for static site generators

## Login

Relies on OIDC login (tested with kanidm).


```
kanidm system oauth2 update-scope-map w2z <group> openid profile email
kanidm system oauth2 update-sup-scope-map w2z <group> group groups scopes
```
