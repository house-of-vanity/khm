class SSHKeyManager {
    constructor() {
        this.currentFlow = null;
        this.keys = [];
        this.filteredKeys = [];
        this.groupedKeys = {};
        this.expandedGroups = new Set();
        this.currentPage = 1;
        this.keysPerPage = 20;
        this.serversPerPage = 10;
        this.selectedKeys = new Set();
        this.showDeprecatedOnly = false;
        
        this.initializeEventListeners();
        this.loadFlows();
    }

    initializeEventListeners() {
        // Flow selection
        document.getElementById('flowSelect').addEventListener('change', (e) => {
            this.currentFlow = e.target.value;
            if (this.currentFlow) {
                this.loadKeys();
            } else {
                this.clearTable();
            }
        });

        // Refresh button
        document.getElementById('refreshBtn').addEventListener('click', () => {
            this.loadFlows();
            if (this.currentFlow) {
                this.loadKeys();
            }
        });

        // Add key button
        document.getElementById('addKeyBtn').addEventListener('click', () => {
            this.showAddKeyModal();
        });

        // Bulk delete button
        document.getElementById('bulkDeleteBtn').addEventListener('click', () => {
            this.deleteSelectedKeys();
        });

        // Bulk permanent delete button
        document.getElementById('bulkPermanentDeleteBtn').addEventListener('click', () => {
            this.permanentlyDeleteSelectedKeys();
        });

        // Search input
        document.getElementById('searchInput').addEventListener('input', (e) => {
            this.filterKeys(e.target.value);
        });

        // Deprecated filter checkbox
        document.getElementById('showDeprecatedOnly').addEventListener('change', (e) => {
            this.showDeprecatedOnly = e.target.checked;
            
            // Update visual state
            const filterLabel = e.target.closest('.filter-label');
            if (e.target.checked) {
                filterLabel.classList.add('active');
            } else {
                filterLabel.classList.remove('active');
            }
            
            this.filterKeys(document.getElementById('searchInput').value);
        });

        // Select all checkbox
        document.getElementById('selectAll').addEventListener('change', (e) => {
            this.toggleSelectAll(e.target.checked);
        });

        // Pagination
        document.getElementById('prevPage').addEventListener('click', () => {
            this.changePage(this.currentPage - 1);
        });

        document.getElementById('nextPage').addEventListener('click', () => {
            this.changePage(this.currentPage + 1);
        });

        // Modal events
        this.initializeModalEvents();
    }

    initializeModalEvents() {
        // Add key modal
        const addModal = document.getElementById('addKeyModal');
        const addForm = document.getElementById('addKeyForm');
        
        addForm.addEventListener('submit', (e) => {
            e.preventDefault();
            this.addKey();
        });

        document.getElementById('cancelAdd').addEventListener('click', () => {
            this.hideModal('addKeyModal');
        });

        // View key modal
        document.getElementById('closeView').addEventListener('click', () => {
            this.hideModal('viewKeyModal');
        });

        document.getElementById('copyKey').addEventListener('click', () => {
            this.copyKeyToClipboard();
        });

        // Close modals when clicking on close button or outside
        document.querySelectorAll('.modal .close').forEach(closeBtn => {
            closeBtn.addEventListener('click', (e) => {
                this.hideModal(e.target.closest('.modal').id);
            });
        });

        document.querySelectorAll('.modal').forEach(modal => {
            modal.addEventListener('click', (e) => {
                if (e.target === modal) {
                    this.hideModal(modal.id);
                }
            });
        });
    }

    async loadFlows() {
        try {
            this.showLoading();
            const response = await fetch('/api/flows');
            if (!response.ok) throw new Error('Failed to load flows');
            
            const flows = await response.json();
            this.populateFlowSelector(flows);
        } catch (error) {
            this.showToast('Failed to load flows: ' + error.message, 'error');
        } finally {
            this.hideLoading();
        }
    }

    populateFlowSelector(flows) {
        const select = document.getElementById('flowSelect');
        select.innerHTML = '<option value="">Select a flow...</option>';
        
        flows.forEach(flow => {
            const option = document.createElement('option');
            option.value = flow;
            option.textContent = flow;
            select.appendChild(option);
        });
        
        // Auto-select the first flow if available
        if (flows.length > 0) {
            select.value = flows[0];
            this.currentFlow = flows[0];
            this.loadKeys();
        }
    }

    async loadKeys() {
        if (!this.currentFlow) return;

        try {
            this.showLoading();
            const response = await fetch(`/${this.currentFlow}/keys?include_deprecated=true`);
            if (!response.ok) throw new Error('Failed to load keys');
            
            this.keys = await response.json();
            this.groupKeys();
            this.filterKeys();
            this.updateStats();
            this.renderTable();
            this.selectedKeys.clear();
            this.updateBulkDeleteButton();
        } catch (error) {
            this.showToast('Failed to load keys: ' + error.message, 'error');
            this.clearTable();
        } finally {
            this.hideLoading();
        }
    }

    groupKeys() {
        this.groupedKeys = {};
        this.keys.forEach(key => {
            if (!this.groupedKeys[key.server]) {
                this.groupedKeys[key.server] = [];
            }
            this.groupedKeys[key.server].push(key);
        });
        
        // Groups are closed by default - no auto-expand
    }

    filterKeys(searchTerm) {
        let keys = [...this.keys];
        
        // Apply deprecated filter first
        if (this.showDeprecatedOnly) {
            keys = keys.filter(key => key.deprecated);
        }
        
        // Then apply search filter
        if (!searchTerm || !searchTerm.trim()) {
            this.filteredKeys = keys;
        } else {
            const term = searchTerm.toLowerCase();
            this.filteredKeys = keys.filter(key => 
                key.server.toLowerCase().includes(term) || 
                key.public_key.toLowerCase().includes(term)
            );
        }
        
        this.currentPage = 1;
        this.renderTable();
    }

    updateStats() {
        document.getElementById('totalKeys').textContent = this.keys.length;
        
        const uniqueServers = new Set(this.keys.map(key => key.server));
        document.getElementById('uniqueServers').textContent = uniqueServers.size;
    }

    getGroupedFilteredKeys() {
        const groupedFilteredKeys = {};
        this.filteredKeys.forEach(key => {
            if (!groupedFilteredKeys[key.server]) {
                groupedFilteredKeys[key.server] = [];
            }
            groupedFilteredKeys[key.server].push(key);
        });
        return groupedFilteredKeys;
    }

    renderTable() {
        const tbody = document.getElementById('keysTableBody');
        const noKeysMessage = document.getElementById('noKeysMessage');
        
        if (this.filteredKeys.length === 0) {
            tbody.innerHTML = '';
            noKeysMessage.style.display = 'block';
            this.updatePagination();
            return;
        }

        noKeysMessage.style.display = 'none';
        
        // Group filtered keys by server
        const groupedFilteredKeys = this.getGroupedFilteredKeys();
        
        // Calculate pagination for grouped view
        const servers = Object.keys(groupedFilteredKeys).sort();
        
        // For pagination, we'll show a reasonable number of server groups per page
        const startServerIndex = (this.currentPage - 1) * this.serversPerPage;
        const endServerIndex = startServerIndex + this.serversPerPage;
        const pageServers = servers.slice(startServerIndex, endServerIndex);
        
        let html = '';
        
        pageServers.forEach(server => {
            const serverKeys = groupedFilteredKeys[server];
            const activeCount = serverKeys.filter(k => !k.deprecated).length;
            const deprecatedCount = serverKeys.filter(k => k.deprecated).length;
            const isExpanded = this.expandedGroups.has(server);
            
            // Server group header
            html += `
                <tr class="host-group-header ${isExpanded ? '' : 'collapsed'}">
                    <td>
                        <input type="checkbox" 
                               data-group="${this.escapeHtml(server)}" 
                               onchange="sshKeyManager.toggleGroupSelection('${this.escapeHtml(server)}', this.checked)"
                               onclick="event.stopPropagation()">
                    </td>
                    <td colspan="4" onclick="sshKeyManager.toggleGroup('${this.escapeHtml(server)}')" style="cursor: pointer;">
                        <span class="expand-icon">${isExpanded ? '▼' : '▶'}</span>
                        <strong>${this.escapeHtml(server)}</strong>
                        <span class="host-summary">
                            <span class="key-count">${serverKeys.length} keys</span>
                            ${deprecatedCount > 0 ? `<span class="deprecated-count">${deprecatedCount} deprecated</span>` : ''}
                        </span>
                    </td>
                </tr>
            `;
            
            // Server keys (if expanded)
            if (isExpanded) {
                serverKeys.forEach(key => {
                    const keyType = this.getKeyType(key.public_key);
                    const keyPreview = this.getKeyPreview(key.public_key);
                    const keyId = `${key.server}-${key.public_key}`;
                    
                    html += `
                        <tr class="key-row${key.deprecated ? ' deprecated' : ''}">
                            <td>
                                <input type="checkbox" data-key-id="${keyId}" ${this.selectedKeys.has(keyId) ? 'checked' : ''}>
                            </td>
                            <td style="padding-left: 2rem;">
                                <span class="key-type ${keyType.toLowerCase()}">${keyType}</span>
                                ${key.deprecated ? '<span class="deprecated-badge">DEPRECATED</span>' : ''}
                            </td>
                            <td><span class="key-preview">${keyPreview}</span></td>
                            <td></td>
                            <td class="table-actions">
                                <button class="btn btn-sm btn-secondary" onclick="sshKeyManager.viewKey('${keyId}')">View</button>
                                ${key.deprecated ? 
                                    `<button class="btn btn-sm btn-success" onclick="sshKeyManager.restoreKey('${keyId}')">Restore</button>
                                     <button class="btn btn-sm btn-danger" onclick="sshKeyManager.permanentlyDeleteKey('${keyId}')">Delete</button>` : 
                                    `<button class="btn btn-sm btn-danger" onclick="sshKeyManager.deleteKey('${keyId}')">Deprecate</button>`
                                }
                            </td>
                        </tr>
                    `;
                });
            }
        });
        
        tbody.innerHTML = html;

        // Add event listeners for checkboxes
        tbody.querySelectorAll('input[type="checkbox"]').forEach(checkbox => {
            checkbox.addEventListener('change', (e) => {
                const keyId = e.target.dataset.keyId;
                if (e.target.checked) {
                    this.selectedKeys.add(keyId);
                } else {
                    this.selectedKeys.delete(keyId);
                }
                this.updateBulkDeleteButton();
                this.updateSelectAllCheckbox();
                this.updateGroupCheckboxes(); // Update group checkboxes when individual keys change
            });
        });

        // Update group checkboxes to show correct indeterminate state
        this.updateGroupCheckboxes();
        this.updatePagination();
    }

    toggleGroup(server) {
        if (this.expandedGroups.has(server)) {
            this.expandedGroups.delete(server);
        } else {
            this.expandedGroups.add(server);
        }
        this.renderTable();
    }

    toggleGroupSelection(server, isChecked) {
        const groupedFilteredKeys = this.getGroupedFilteredKeys();
        const serverKeys = groupedFilteredKeys[server] || [];
        
        serverKeys.forEach(key => {
            const keyId = `${key.server}-${key.public_key}`;
            if (isChecked) {
                this.selectedKeys.add(keyId);
            } else {
                this.selectedKeys.delete(keyId);
            }
        });
        
        this.updateBulkDeleteButton();
        this.updateSelectAllCheckbox();
        this.updateGroupCheckboxes();
        
        // Update individual checkboxes without full re-render
        const tbody = document.getElementById('keysTableBody');
        serverKeys.forEach(key => {
            const keyId = `${key.server}-${key.public_key}`;
            const checkbox = tbody.querySelector(`input[data-key-id="${keyId}"]`);
            if (checkbox) {
                checkbox.checked = this.selectedKeys.has(keyId);
            }
        });
    }

    updateGroupCheckboxes() {
        const groupedFilteredKeys = this.getGroupedFilteredKeys();
        const tbody = document.getElementById('keysTableBody');
        
        Object.keys(groupedFilteredKeys).forEach(server => {
            const serverKeys = groupedFilteredKeys[server];
            const groupCheckbox = tbody.querySelector(`input[data-group="${server}"]`);
            
            if (groupCheckbox) {
                const allSelected = serverKeys.every(key => 
                    this.selectedKeys.has(`${key.server}-${key.public_key}`)
                );
                const someSelected = serverKeys.some(key => 
                    this.selectedKeys.has(`${key.server}-${key.public_key}`)
                );
                
                groupCheckbox.checked = allSelected;
                groupCheckbox.indeterminate = someSelected && !allSelected;
            }
        });
    }

    updatePagination() {
        const groupedFilteredKeys = this.getGroupedFilteredKeys();
        const totalServers = Object.keys(groupedFilteredKeys).length;
        const totalPages = Math.ceil(totalServers / this.serversPerPage);
        
        document.getElementById('pageInfo').textContent = `Page ${this.currentPage} of ${totalPages}`;
        document.getElementById('prevPage').disabled = this.currentPage <= 1;
        document.getElementById('nextPage').disabled = this.currentPage >= totalPages;
    }

    changePage(newPage) {
        const groupedFilteredKeys = this.getGroupedFilteredKeys();
        const totalServers = Object.keys(groupedFilteredKeys).length;
        const totalPages = Math.ceil(totalServers / this.serversPerPage);
        
        if (newPage >= 1 && newPage <= totalPages) {
            this.currentPage = newPage;
            this.renderTable();
        }
    }

    toggleSelectAll(checked) {
        this.selectedKeys.clear();
        
        if (checked) {
            this.filteredKeys.forEach(key => {
                const keyId = `${key.server}-${key.public_key}`;
                this.selectedKeys.add(keyId);
            });
        }
        
        this.renderTable();
        this.updateBulkDeleteButton();
    }

    updateSelectAllCheckbox() {
        const selectAllCheckbox = document.getElementById('selectAll');
        const visibleKeys = this.filteredKeys.length;
        const selectedVisibleKeys = this.filteredKeys.filter(key => 
            this.selectedKeys.has(`${key.server}-${key.public_key}`)
        ).length;
        
        if (selectedVisibleKeys === 0) {
            selectAllCheckbox.indeterminate = false;
            selectAllCheckbox.checked = false;
        } else if (selectedVisibleKeys === visibleKeys) {
            selectAllCheckbox.indeterminate = false;
            selectAllCheckbox.checked = true;
        } else {
            selectAllCheckbox.indeterminate = true;
        }
    }

    updateBulkDeleteButton() {
        const bulkDeleteBtn = document.getElementById('bulkDeleteBtn');
        const bulkPermanentDeleteBtn = document.getElementById('bulkPermanentDeleteBtn');
        
        if (this.selectedKeys.size === 0) {
            // No keys selected - hide both buttons
            bulkDeleteBtn.disabled = true;
            bulkDeleteBtn.textContent = 'Deprecate Selected';
            bulkPermanentDeleteBtn.style.display = 'none';
            bulkPermanentDeleteBtn.disabled = true;
            return;
        }

        // Count selected active and deprecated keys
        let activeCount = 0;
        let deprecatedCount = 0;
        
        Array.from(this.selectedKeys).forEach(keyId => {
            const key = this.findKeyById(keyId);
            if (key) {
                if (key.deprecated) {
                    deprecatedCount++;
                } else {
                    activeCount++;
                }
            }
        });

        // Show/hide deprecate button
        if (activeCount > 0) {
            bulkDeleteBtn.disabled = false;
            bulkDeleteBtn.textContent = `Deprecate Selected (${activeCount})`;
        } else {
            bulkDeleteBtn.disabled = true;
            bulkDeleteBtn.textContent = 'Deprecate Selected';
        }

        // Show/hide permanent delete button
        if (deprecatedCount > 0) {
            bulkPermanentDeleteBtn.style.display = 'inline-flex';
            bulkPermanentDeleteBtn.disabled = false;
            bulkPermanentDeleteBtn.textContent = `Delete Selected (${deprecatedCount})`;
        } else {
            bulkPermanentDeleteBtn.style.display = 'none';
            bulkPermanentDeleteBtn.disabled = true;
        }
    }

    showAddKeyModal() {
        if (!this.currentFlow) {
            this.showToast('Please select a flow first', 'warning');
            return;
        }
        
        document.getElementById('serverInput').value = '';
        document.getElementById('keyInput').value = '';
        this.showModal('addKeyModal');
    }

    async addKey() {
        const server = document.getElementById('serverInput').value.trim();
        const publicKey = document.getElementById('keyInput').value.trim();
        
        if (!server || !publicKey) {
            this.showToast('Please fill in all fields', 'warning');
            return;
        }

        if (!this.validateSSHKey(publicKey)) {
            this.showToast('Invalid SSH key format', 'error');
            return;
        }

        try {
            this.showLoading();
            const response = await fetch(`/${this.currentFlow}/keys`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify([{
                    server: server,
                    public_key: publicKey
                }])
            });

            if (!response.ok) {
                const errorText = await response.text();
                throw new Error(errorText || 'Failed to add key');
            }

            this.hideModal('addKeyModal');
            this.showToast('SSH key added successfully', 'success');
            await this.loadKeys();
        } catch (error) {
            this.showToast('Failed to add key: ' + error.message, 'error');
        } finally {
            this.hideLoading();
        }
    }

    viewKey(keyId) {
        const key = this.findKeyById(keyId);
        if (!key) return;

        document.getElementById('viewServer').textContent = key.server;
        document.getElementById('viewKey').value = key.public_key;
        this.showModal('viewKeyModal');
    }

    async deleteKey(keyId) {
        const key = this.findKeyById(keyId);
        if (!key) return;
        
        if (key.deprecated) {
            this.showToast('This key is already deprecated', 'warning');
            return;
        }

        if (!confirm('Are you sure you want to deprecate this SSH key?')) {
            return;
        }

        try {
            this.showLoading();
            const response = await fetch(`/${this.currentFlow}/keys/${encodeURIComponent(key.server)}`, {
                method: 'DELETE'
            });

            if (!response.ok) {
                const errorText = await response.text();
                throw new Error(errorText || 'Failed to deprecate key');
            }

            this.showToast('SSH key deprecated successfully', 'success');
            await this.loadKeys();
        } catch (error) {
            this.showToast('Failed to deprecate key: ' + error.message, 'error');
        } finally {
            this.hideLoading();
        }
    }

    async restoreKey(keyId) {
        const key = this.findKeyById(keyId);
        if (!key) return;
        
        if (!key.deprecated) {
            this.showToast('This key is not deprecated', 'warning');
            return;
        }

        if (!confirm('Are you sure you want to restore this SSH key from deprecated status?')) {
            return;
        }

        try {
            this.showLoading();
            const response = await fetch(`/${this.currentFlow}/keys/${encodeURIComponent(key.server)}/restore`, {
                method: 'POST'
            });

            if (!response.ok) {
                const errorText = await response.text();
                throw new Error(errorText || 'Failed to restore key');
            }

            this.showToast('SSH key restored successfully', 'success');
            await this.loadKeys();
        } catch (error) {
            this.showToast('Failed to restore key: ' + error.message, 'error');
        } finally {
            this.hideLoading();
        }
    }

    async permanentlyDeleteKey(keyId) {
        const key = this.findKeyById(keyId);
        if (!key) return;

        if (!confirm('⚠️ Are you sure you want to PERMANENTLY DELETE this SSH key?\n\nThis action cannot be undone!')) {
            return;
        }

        // Double confirmation for permanent deletion
        if (!confirm('This will permanently remove the key from the database.\n\nConfirm permanent deletion?')) {
            return;
        }

        try {
            this.showLoading();
            const response = await fetch(`/${this.currentFlow}/keys/${encodeURIComponent(key.server)}/delete`, {
                method: 'DELETE'
            });

            if (!response.ok) {
                const errorText = await response.text();
                throw new Error(errorText || 'Failed to permanently delete key');
            }

            this.showToast('SSH key permanently deleted', 'success');
            await this.loadKeys();
        } catch (error) {
            this.showToast('Failed to permanently delete key: ' + error.message, 'error');
        } finally {
            this.hideLoading();
        }
    }

    async deleteSelectedKeys() {
        if (this.selectedKeys.size === 0) return;

        // Filter out already deprecated keys
        const activeKeys = Array.from(this.selectedKeys).filter(keyId => {
            const key = this.findKeyById(keyId);
            return key && !key.deprecated;
        });

        if (activeKeys.length === 0) {
            this.showToast('All selected keys are already deprecated', 'warning');
            return;
        }

        if (!confirm(`Are you sure you want to deprecate ${activeKeys.length} selected SSH keys?`)) {
            return;
        }

        try {
            this.showLoading();
            
            const deprecatePromises = activeKeys.map(keyId => {
                const key = this.findKeyById(keyId);
                if (!key) return Promise.resolve();
                
                return fetch(`/${this.currentFlow}/keys/${encodeURIComponent(key.server)}`, {
                    method: 'DELETE'
                });
            });

            await Promise.all(deprecatePromises);
            this.showToast(`${activeKeys.length} SSH keys deprecated successfully`, 'success');
            await this.loadKeys();
        } catch (error) {
            this.showToast('Failed to deprecate selected keys: ' + error.message, 'error');
        } finally {
            this.hideLoading();
        }
    }

    async permanentlyDeleteSelectedKeys() {
        if (this.selectedKeys.size === 0) return;

        // Filter only deprecated keys
        const deprecatedKeys = Array.from(this.selectedKeys).filter(keyId => {
            const key = this.findKeyById(keyId);
            return key && key.deprecated;
        });

        if (deprecatedKeys.length === 0) {
            this.showToast('No deprecated keys selected', 'warning');
            return;
        }

        if (!confirm(`⚠️ Are you sure you want to PERMANENTLY DELETE ${deprecatedKeys.length} deprecated SSH keys?\n\nThis action cannot be undone!`)) {
            return;
        }

        // Double confirmation for permanent deletion
        if (!confirm('This will permanently remove the keys from the database.\n\nConfirm permanent deletion?')) {
            return;
        }

        try {
            this.showLoading();
            
            const deletePromises = deprecatedKeys.map(keyId => {
                const key = this.findKeyById(keyId);
                if (!key) return Promise.resolve();
                
                return fetch(`/${this.currentFlow}/keys/${encodeURIComponent(key.server)}/delete`, {
                    method: 'DELETE'
                });
            });

            await Promise.all(deletePromises);
            this.showToast(`${deprecatedKeys.length} SSH keys permanently deleted`, 'success');
            await this.loadKeys();
        } catch (error) {
            this.showToast('Failed to permanently delete selected keys: ' + error.message, 'error');
        } finally {
            this.hideLoading();
        }
    }

    findKeyById(keyId) {
        return this.keys.find(key => `${key.server}-${key.public_key}` === keyId);
    }

    validateSSHKey(key) {
        const sshKeyRegex = /^(ssh-rsa|ssh-dss|ecdsa-sha2-nistp(256|384|521)|ssh-ed25519)\s+[A-Za-z0-9+/]+=*(\s+.*)?$/;
        return sshKeyRegex.test(key.trim());
    }

    getKeyType(publicKey) {
        if (publicKey.startsWith('ssh-rsa')) return 'RSA';
        if (publicKey.startsWith('ssh-ed25519')) return 'ED25519';
        if (publicKey.startsWith('ecdsa-sha2-nistp')) return 'ECDSA';
        if (publicKey.startsWith('ssh-dss')) return 'DSA';
        return 'Unknown';
    }

    getKeyPreview(publicKey) {
        const parts = publicKey.split(' ');
        if (parts.length >= 2) {
            const keyPart = parts[1];
            if (keyPart.length > 20) {
                return keyPart.substring(0, 20) + '...';
            }
            return keyPart;
        }
        return publicKey.substring(0, 20) + '...';
    }

    escapeHtml(text) {
        const map = {
            '&': '&amp;',
            '<': '&lt;',
            '>': '&gt;',
            '"': '&quot;',
            "'": '&#039;'
        };
        return text.replace(/[&<>"']/g, (m) => map[m]);
    }

    copyKeyToClipboard() {
        const keyTextarea = document.getElementById('viewKey');
        keyTextarea.select();
        keyTextarea.setSelectionRange(0, 99999);
        
        try {
            document.execCommand('copy');
            this.showToast('SSH key copied to clipboard', 'success');
        } catch (error) {
            this.showToast('Failed to copy to clipboard', 'error');
        }
    }

    clearTable() {
        document.getElementById('keysTableBody').innerHTML = '';
        document.getElementById('noKeysMessage').style.display = 'block';
        document.getElementById('totalKeys').textContent = '0';
        document.getElementById('uniqueServers').textContent = '0';
        this.selectedKeys.clear();
        this.updateBulkDeleteButton();
    }

    showModal(modalId) {
        document.getElementById(modalId).style.display = 'block';
        document.body.style.overflow = 'hidden';
    }

    hideModal(modalId) {
        document.getElementById(modalId).style.display = 'none';
        document.body.style.overflow = 'auto';
    }

    showLoading() {
        document.getElementById('loadingOverlay').style.display = 'block';
    }

    hideLoading() {
        document.getElementById('loadingOverlay').style.display = 'none';
    }

    showToast(message, type = 'info') {
        const toastContainer = document.getElementById('toastContainer');
        const toast = document.createElement('div');
        toast.className = `toast ${type}`;
        toast.textContent = message;
        
        toastContainer.appendChild(toast);
        
        // Trigger animation
        setTimeout(() => toast.classList.add('show'), 100);
        
        // Remove toast after 4 seconds
        setTimeout(() => {
            toast.classList.remove('show');
            setTimeout(() => {
                if (toast.parentNode) {
                    toast.parentNode.removeChild(toast);
                }
            }, 300);
        }, 4000);
    }
}

// Initialize the SSH Key Manager when the page loads
document.addEventListener('DOMContentLoaded', () => {
    window.sshKeyManager = new SSHKeyManager();
});
