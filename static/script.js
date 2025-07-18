class SSHKeyManager {
    constructor() {
        this.currentFlow = null;
        this.keys = [];
        this.filteredKeys = [];
        this.currentPage = 1;
        this.keysPerPage = 20;
        this.selectedKeys = new Set();
        
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

        // Search input
        document.getElementById('searchInput').addEventListener('input', (e) => {
            this.filterKeys(e.target.value);
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
    }

    async loadKeys() {
        if (!this.currentFlow) return;

        try {
            this.showLoading();
            const response = await fetch(`/${this.currentFlow}/keys`);
            if (!response.ok) throw new Error('Failed to load keys');
            
            this.keys = await response.json();
            this.filteredKeys = [...this.keys];
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

    filterKeys(searchTerm) {
        if (!searchTerm.trim()) {
            this.filteredKeys = [...this.keys];
        } else {
            const term = searchTerm.toLowerCase();
            this.filteredKeys = this.keys.filter(key => 
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
        
        const startIndex = (this.currentPage - 1) * this.keysPerPage;
        const endIndex = startIndex + this.keysPerPage;
        const pageKeys = this.filteredKeys.slice(startIndex, endIndex);
        
        tbody.innerHTML = pageKeys.map((key, index) => {
            const keyType = this.getKeyType(key.public_key);
            const keyPreview = this.getKeyPreview(key.public_key);
            const keyId = `${key.server}-${key.public_key}`;
            
            return `
                <tr>
                    <td>
                        <input type="checkbox" data-key-id="${keyId}" ${this.selectedKeys.has(keyId) ? 'checked' : ''}>
                    </td>
                    <td>${this.escapeHtml(key.server)}</td>
                    <td><span class="key-type ${keyType.toLowerCase()}">${keyType}</span></td>
                    <td><span class="key-preview">${keyPreview}</span></td>
                    <td class="table-actions">
                        <button class="btn btn-sm btn-secondary" onclick="sshKeyManager.viewKey('${keyId}')">View</button>
                        <button class="btn btn-sm btn-danger" onclick="sshKeyManager.deleteKey('${keyId}')">Delete</button>
                    </td>
                </tr>
            `;
        }).join('');

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
            });
        });

        this.updatePagination();
    }

    updatePagination() {
        const totalPages = Math.ceil(this.filteredKeys.length / this.keysPerPage);
        
        document.getElementById('pageInfo').textContent = `Page ${this.currentPage} of ${totalPages}`;
        document.getElementById('prevPage').disabled = this.currentPage <= 1;
        document.getElementById('nextPage').disabled = this.currentPage >= totalPages;
    }

    changePage(newPage) {
        const totalPages = Math.ceil(this.filteredKeys.length / this.keysPerPage);
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
        bulkDeleteBtn.disabled = this.selectedKeys.size === 0;
        bulkDeleteBtn.textContent = this.selectedKeys.size > 0 
            ? `Delete Selected (${this.selectedKeys.size})` 
            : 'Delete Selected';
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
        if (!confirm('Are you sure you want to delete this SSH key?')) {
            return;
        }

        const key = this.findKeyById(keyId);
        if (!key) return;

        try {
            this.showLoading();
            const response = await fetch(`/${this.currentFlow}/keys/${encodeURIComponent(key.server)}`, {
                method: 'DELETE'
            });

            if (!response.ok) {
                const errorText = await response.text();
                throw new Error(errorText || 'Failed to delete key');
            }

            this.showToast('SSH key deleted successfully', 'success');
            await this.loadKeys();
        } catch (error) {
            this.showToast('Failed to delete key: ' + error.message, 'error');
        } finally {
            this.hideLoading();
        }
    }

    async deleteSelectedKeys() {
        if (this.selectedKeys.size === 0) return;

        if (!confirm(`Are you sure you want to delete ${this.selectedKeys.size} selected SSH keys?`)) {
            return;
        }

        try {
            this.showLoading();
            
            const deletePromises = Array.from(this.selectedKeys).map(keyId => {
                const key = this.findKeyById(keyId);
                if (!key) return Promise.resolve();
                
                return fetch(`/${this.currentFlow}/keys/${encodeURIComponent(key.server)}`, {
                    method: 'DELETE'
                });
            });

            await Promise.all(deletePromises);
            this.showToast(`${this.selectedKeys.size} SSH keys deleted successfully`, 'success');
            await this.loadKeys();
        } catch (error) {
            this.showToast('Failed to delete selected keys: ' + error.message, 'error');
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
